use std::fmt;
use std::future::Future;
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use thiserror::Error;

use wasmer::Store;
pub use wasmer_vfs::StdioMode;
use wasmer_vfs::VirtualFile;
use wasmer_wasi_types::wasi::{BusDataFormat, ExitCode};

pub type Result<T> = std::result::Result<T, VirtualBusError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct CallDescriptor(u32);

impl CallDescriptor {
    pub fn raw(&self) -> u32 {
        self.0
    }
}

impl From<u32> for CallDescriptor {
    fn from(a: u32) -> Self {
        Self(a)
    }
}

pub trait VirtualBus<T>: fmt::Debug + Send + Sync + 'static
where
    T: SpawnEnvironmentIntrinsics,
    T: std::fmt::Debug + Send + Sync + 'static,
{
    /// Starts a new WAPM sub process
    fn spawn(&self, env: T) -> SpawnOptions<T> {
        SpawnOptions::new(Box::new(DefaultVirtualBusSpawner::default()), env)
    }

    /// Creates a listener thats used to receive BUS commands
    fn listen<'a>(&'a self) -> Result<&'a dyn VirtualBusListener> {
        Err(VirtualBusError::Unsupported)
    }
}

pub trait VirtualBusSpawner<T> {
    /// Spawns a new WAPM process by its name
    fn spawn<'a>(
        &'a self,
        name: String,
        store: Store,
        config: SpawnOptionsConfig<T>,
        fallback: Box<dyn VirtualBusSpawner<T>>,
    ) -> Pin<Box<dyn Future<Output = Result<BusSpawnedProcess>> + 'a>>
    where
        T: 'static,
    {
        Box::pin(async move {
            let fallback_inner = Box::new(UnsupportedVirtualBusSpawner::default());
            fallback.spawn(name, store, config, fallback_inner).await
        })
    }
}

#[derive(Debug, Default)]
pub struct UnsupportedVirtualBusSpawner {}
impl<T> VirtualBusSpawner<T> for UnsupportedVirtualBusSpawner {
    fn spawn(
        &self,
        _name: String,
        _store: Store,
        _config: SpawnOptionsConfig<T>,
        _fallback: Box<dyn VirtualBusSpawner<T>>,
    ) -> Pin<Box<dyn Future<Output = Result<BusSpawnedProcess>>>> {
        Box::pin(async move { Err(VirtualBusError::Unsupported) })
    }
}

#[derive(Debug, Clone)]
pub struct SpawnOptionsConfig<T> {
    pub reuse: bool,
    pub env: T,
    pub remote_instance: Option<String>,
    pub access_token: Option<String>,
}

pub trait SpawnEnvironmentIntrinsics {
    fn args(&self) -> &Vec<String>;

    fn preopen(&self) -> &Vec<String>;

    fn stdin_mode(&self) -> StdioMode;

    fn stdout_mode(&self) -> StdioMode;

    fn stderr_mode(&self) -> StdioMode;

    fn working_dir(&self) -> String;
}

impl<T> SpawnOptionsConfig<T>
where
    T: SpawnEnvironmentIntrinsics,
{
    pub fn reuse(&self) -> bool {
        self.reuse
    }

    pub fn env(&self) -> &T {
        &self.env
    }

    pub fn env_mut(&mut self) -> &mut T {
        &mut self.env
    }

    pub fn remote_instance(&self) -> Option<&str> {
        self.remote_instance.as_deref()
    }

    pub fn access_token(&self) -> Option<&str> {
        self.access_token.as_deref()
    }
}

pub struct SpawnOptions<T> {
    spawner: Box<dyn VirtualBusSpawner<T>>,
    conf: SpawnOptionsConfig<T>,
}

impl<T> SpawnOptions<T>
where
    T: SpawnEnvironmentIntrinsics,
{
    pub fn new(spawner: Box<dyn VirtualBusSpawner<T>>, env: T) -> Self {
        Self {
            spawner,
            conf: SpawnOptionsConfig {
                reuse: false,
                env,
                remote_instance: None,
                access_token: None,
            },
        }
    }

    pub fn conf(self) -> SpawnOptionsConfig<T> {
        self.conf
    }

    pub fn conf_ref(&self) -> &SpawnOptionsConfig<T> {
        &self.conf
    }

    pub fn options(mut self, options: SpawnOptionsConfig<T>) -> Self {
        self.conf = options;
        self
    }

    /// Spawns a new bus instance by its reference name
    pub fn spawn(
        self,
        name: String,
        store: Store,
        fallback: Box<dyn VirtualBusSpawner<T>>,
    ) -> Pin<Box<dyn Future<Output = Result<BusSpawnedProcess>> + 'static>>
    where
        T: 'static,
    {
        Box::pin(async move { self.spawner.spawn(name, store, self.conf, fallback).await })
    }
}

enum BusSpawnedProcessJoinResult {
    Active(Box<dyn VirtualBusProcess + Sync + Unpin>),
    Finished(Option<ExitCode>),
}

#[derive(Clone)]
pub struct BusSpawnedProcessJoin {
    inst: Arc<Mutex<BusSpawnedProcessJoinResult>>,
}

impl BusSpawnedProcessJoin {
    pub fn new(process: BusSpawnedProcess) -> Self {
        Self {
            inst: Arc::new(Mutex::new(BusSpawnedProcessJoinResult::Active(
                process.inst,
            ))),
        }
    }

    pub fn poll_finished(&self, cx: &mut Context<'_>) -> Poll<Option<ExitCode>> {
        let mut guard = self.inst.lock().unwrap();
        match guard.deref_mut() {
            BusSpawnedProcessJoinResult::Active(inst) => {
                let pinned_inst = Pin::new(inst.as_mut());
                match pinned_inst.poll_ready(cx) {
                    Poll::Ready(_) => {
                        let exit_code = inst.exit_code();
                        let mut swap = BusSpawnedProcessJoinResult::Finished(exit_code);
                        std::mem::swap(guard.deref_mut(), &mut swap);
                        Poll::Ready(exit_code)
                    }
                    Poll::Pending => Poll::Pending,
                }
            }
            BusSpawnedProcessJoinResult::Finished(exit_code) => Poll::Ready(*exit_code),
        }
    }
}

impl Future for BusSpawnedProcessJoin {
    type Output = Option<ExitCode>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_finished(cx)
    }
}

/// Signal handles...well...they process signals
pub trait SignalHandlerAbi
where
    Self: std::fmt::Debug,
{
    /// Processes a signal
    fn signal(&self, sig: u8);
}

#[derive(Debug)]
pub struct BusSpawnedProcess {
    /// Reference to the spawned instance
    pub inst: Box<dyn VirtualBusProcess + Sync + Unpin>,
    /// Virtual file used for stdin
    pub stdin: Option<Box<dyn VirtualFile + Send + Sync + 'static>>,
    /// Virtual file used for stdout
    pub stdout: Option<Box<dyn VirtualFile + Send + Sync + 'static>>,
    /// Virtual file used for stderr
    pub stderr: Option<Box<dyn VirtualFile + Send + Sync + 'static>>,
    /// The signal handler for this process (if any)
    pub signaler: Option<Box<dyn SignalHandlerAbi + Send + Sync + 'static>>,
}

impl BusSpawnedProcess {
    pub fn exited_process(exit_code: ExitCode) -> Self {
        Self {
            inst: Box::new(ExitedProcess { exit_code }),
            stdin: None,
            stdout: None,
            stderr: None,
            signaler: None,
        }
    }
}

pub trait VirtualBusScope: fmt::Debug + Send + Sync + 'static {
    //// Returns true if the invokable target has finished
    fn poll_finished(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()>;
}

pub trait VirtualBusInvokable: fmt::Debug + Send + Sync + 'static {
    /// Invokes a service within this instance
    #[allow(unused_variables)]
    fn invoke(
        &self,
        topic_hash: u128,
        format: BusDataFormat,
        buf: Vec<u8>,
    ) -> Box<dyn VirtualBusInvoked> {
        Box::new(UnsupportedBusInvoker::default())
    }
}

#[derive(Debug, Default)]
struct UnsupportedBusInvoker {}

impl VirtualBusInvoked for UnsupportedBusInvoker {
    #[allow(unused_variables)]
    fn poll_invoked(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Box<dyn VirtualBusInvocation + Sync>>> {
        Poll::Ready(Err(VirtualBusError::Unsupported))
    }
}

pub trait VirtualBusInvoked: fmt::Debug + Unpin + 'static {
    //// Returns once the bus has been invoked (or failed)
    fn poll_invoked(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Box<dyn VirtualBusInvocation + Sync>>>;
}
pub struct VirtualBusInvokedWait {
    invoked: Box<dyn VirtualBusInvoked>,
}
impl VirtualBusInvokedWait {
    pub fn new(invoked: Box<dyn VirtualBusInvoked>) -> Self {
        Self { invoked }
    }
}
impl Future for VirtualBusInvokedWait {
    type Output = Result<Box<dyn VirtualBusInvocation + Sync>>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let invoked = Pin::new(self.invoked.deref_mut());
        invoked.poll_invoked(cx)
    }
}

pub trait VirtualBusProcess:
    VirtualBusScope + VirtualBusInvokable + fmt::Debug + Send + Sync + 'static
{
    /// Returns the exit code if the instance has finished
    fn exit_code(&self) -> Option<ExitCode>;

    /// Polls to check if the process is ready yet to receive commands
    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()>;
}

pub trait VirtualBusInvocation:
    VirtualBusInvokable + fmt::Debug + Send + Sync + Unpin + 'static
{
    /// Polls for new listen events related to this context
    fn poll_event(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<BusInvocationEvent>;
}

#[derive(Debug)]
pub struct InstantInvocation {
    val: Option<BusInvocationEvent>,
    err: Option<VirtualBusError>,
    call: Option<Box<dyn VirtualBusInvocation + Sync>>,
}

impl InstantInvocation {
    pub fn response(format: BusDataFormat, data: Vec<u8>) -> Self {
        Self {
            val: Some(BusInvocationEvent::Response { format, data }),
            err: None,
            call: None,
        }
    }

    pub fn fault(err: VirtualBusError) -> Self {
        Self {
            val: None,
            err: Some(err),
            call: None,
        }
    }

    pub fn call(val: Box<dyn VirtualBusInvocation + Sync>) -> Self {
        Self {
            val: None,
            err: None,
            call: Some(val),
        }
    }
}

impl VirtualBusInvoked for InstantInvocation {
    fn poll_invoked(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<Box<dyn VirtualBusInvocation + Sync>>> {
        if let Some(err) = self.err.take() {
            return Poll::Ready(Err(err));
        }
        if let Some(val) = self.val.take() {
            return Poll::Ready(Ok(Box::new(InstantInvocation {
                val: Some(val),
                err: None,
                call: None,
            })));
        }
        match self.call.take() {
            Some(val) => Poll::Ready(Ok(val)),
            None => Poll::Ready(Err(VirtualBusError::AlreadyConsumed)),
        }
    }
}

impl VirtualBusInvocation for InstantInvocation {
    fn poll_event(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<BusInvocationEvent> {
        match self.val.take() {
            Some(val) => Poll::Ready(val),
            None => Poll::Ready(BusInvocationEvent::Fault {
                fault: VirtualBusError::AlreadyConsumed,
            }),
        }
    }
}

impl VirtualBusInvokable for InstantInvocation {
    fn invoke(
        &self,
        _topic_hash: u128,
        _format: BusDataFormat,
        _buf: Vec<u8>,
    ) -> Box<dyn VirtualBusInvoked> {
        Box::new(InstantInvocation {
            val: None,
            err: Some(VirtualBusError::InvalidTopic),
            call: None,
        })
    }
}

#[derive(Debug)]
pub enum BusInvocationEvent {
    /// The server has sent some out-of-band data to you
    Callback {
        /// Topic that this call relates to
        topic_hash: u128,
        /// Format of the data we received
        format: BusDataFormat,
        /// Data passed in the call
        data: Vec<u8>,
    },
    /// The service has a responded to your call
    Response {
        /// Format of the data we received
        format: BusDataFormat,
        /// Data returned by the call
        data: Vec<u8>,
    },
    /// The service has responded with a fault
    Fault {
        /// Fault code that was raised
        fault: VirtualBusError,
    },
}

pub trait VirtualBusListener: fmt::Debug + Send + Sync + Unpin + 'static {
    /// Polls for new calls to this service
    fn poll(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<BusCallEvent>;
}

#[derive(Debug)]
pub struct BusCallEvent {
    /// Topic hash that this call relates to
    pub topic_hash: u128,
    /// Reference to the call itself
    pub called: Box<dyn VirtualBusCalled + Sync + Unpin>,
    /// Format of the data we received
    pub format: BusDataFormat,
    /// Data passed in the call
    pub data: Vec<u8>,
}

pub trait VirtualBusCalled: fmt::Debug + Send + Sync + 'static {
    /// Polls for new calls to this service
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<BusCallEvent>;

    /// Sends an out-of-band message back to the caller
    fn callback(&self, topic_hash: u128, format: BusDataFormat, buf: Vec<u8>);

    /// Informs the caller that their call has failed
    fn fault(self: Box<Self>, fault: VirtualBusError);

    /// Finishes the call and returns a particular response
    fn reply(&self, format: BusDataFormat, buf: Vec<u8>);
}

// /// Format that the supplied data is in
// #[derive(Debug, Copy, Clone, PartialEq, Eq)]
// pub enum BusDataFormat {
//     Raw,
//     Bincode,
//     MessagePack,
//     Json,
//     Yaml,
//     Xml,
//     Rkyv,
// }

#[derive(Debug, Default)]
pub struct DefaultVirtualBus {}

impl<T> VirtualBus<T> for DefaultVirtualBus
where
    T: SpawnEnvironmentIntrinsics,
    T: std::fmt::Debug + Send + Sync + 'static,
{
}

#[derive(Debug, Default)]
pub struct DefaultVirtualBusSpawner {}

impl<T> VirtualBusSpawner<T> for DefaultVirtualBusSpawner where
    T: std::fmt::Debug + Send + Sync + 'static
{
}

#[derive(Error, Copy, Clone, Debug, PartialEq, Eq)]
pub enum VirtualBusError {
    /// Failed during serialization
    #[error("serialization failed")]
    Serialization,
    /// Failed during deserialization
    #[error("deserialization failed")]
    Deserialization,
    /// Invalid WAPM process
    #[error("invalid wapm")]
    InvalidWapm,
    /// Failed to fetch the WAPM process
    #[error("fetch failed")]
    FetchFailed,
    /// Failed to compile the WAPM process
    #[error("compile error")]
    CompileError,
    /// Invalid ABI
    #[error("WAPM process has an invalid ABI")]
    InvalidABI,
    /// Call was aborted
    #[error("call aborted")]
    Aborted,
    /// Bad handle
    #[error("bad handle")]
    BadHandle,
    /// Invalid topic
    #[error("invalid topic")]
    InvalidTopic,
    /// Invalid callback
    #[error("invalid callback")]
    BadCallback,
    /// Call is unsupported
    #[error("unsupported")]
    Unsupported,
    /// Not found
    #[error("not found")]
    NotFound,
    /// Bad request
    #[error("bad request")]
    BadRequest,
    /// Access denied
    #[error("access denied")]
    AccessDenied,
    /// Internal error has occured
    #[error("internal error")]
    InternalError,
    /// Memory allocation failed
    #[error("memory allocation failed")]
    MemoryAllocationFailed,
    /// Invocation has failed
    #[error("invocation has failed")]
    InvokeFailed,
    /// Already consumed
    #[error("already consumed")]
    AlreadyConsumed,
    /// Memory access violation
    #[error("memory access violation")]
    MemoryAccessViolation,
    /// Some other unhandled error. If you see this, it's probably a bug.
    #[error("unknown error found")]
    UnknownError,
}

#[derive(Debug)]
pub struct ExitedProcess {
    pub exit_code: ExitCode,
}

impl VirtualBusProcess for ExitedProcess {
    fn exit_code(&self) -> Option<ExitCode> {
        Some(self.exit_code.clone())
    }

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        Poll::Ready(())
    }
}

impl VirtualBusScope for ExitedProcess {
    fn poll_finished(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        VirtualBusProcess::poll_ready(self, cx)
    }
}

impl VirtualBusInvokable for ExitedProcess {}
