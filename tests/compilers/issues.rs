//! This file is mainly to assure specific issues are working well
use anyhow::Result;
use wasmer::*;

/// Corruption of WasmerEnv when using call indirect.
///
/// Note: this one is specific to Singlepass, but we want to test in all
/// available compilers.
///
/// https://github.com/wasmerio/wasmer/issues/2329
#[compiler_test(issues)]
fn issue_2329(mut config: crate::Config) -> Result<()> {
    let store = config.store();

    #[derive(Clone, Default, WasmerEnv)]
    pub struct Env {
        #[wasmer(export)]
        memory: LazyInit<Memory>,
    }

    impl Env {
        pub fn new() -> Self {
            Self {
                memory: LazyInit::new(),
            }
        }
    }

    pub fn read_memory(env: &Env, guest_ptr: u32) -> u32 {
        dbg!(env.memory_ref());
        dbg!(guest_ptr);
        0
    }

    let wat = r#"
    (module
        (type (;0;) (func (param i32) (result i32)))
        (type (;1;) (func))
        (type (;2;) (func (param i32 i32) (result i32)))
        (import "env" "__read_memory" (func $__read_memory (type 0)))
        (func $read_memory (type 1)
          (drop
            (call $_ZN5other8dispatch17h053cb34ef5d0d7b0E
              (i32.const 1)
              (i32.const 2)))
          (drop
            (call $__read_memory
              (i32.const 1))))
        (func $_ZN5other8dispatch17h053cb34ef5d0d7b0E (type 2) (param i32 i32) (result i32)
          (call_indirect (type 0)
            (local.get 1)
            (local.get 0)))
        (table (;0;) 2 2 funcref)
        (memory (;0;) 16)
        (global (;0;) (mut i32) (i32.const 1048576))
        (global (;1;) i32 (i32.const 1048576))
        (global (;2;) i32 (i32.const 1048576))
        (export "memory" (memory 0))
        (export "read_memory" (func $read_memory))
        (export "__data_end" (global 1))
        (export "__heap_base" (global 2))
        (elem (;0;) (i32.const 1) func $__read_memory))
    "#;
    let module = Module::new(&store, wat)?;
    let env = Env::new();
    let imports: ImportObject = imports! {
        "env" => {
            "__read_memory" => Function::new_native_with_env(
                &store,
                env.clone(),
                read_memory
            ),
        }
    };
    let instance = Instance::new(&module, &imports)?;
    instance.exports.get_function("read_memory")?.call(&[])?;
    Ok(())
}

#[compiler_test(issues)]
fn call_with_static_data_pointers(mut config: crate::Config) -> Result<()> {
    let store = config.store();
    let memory = Memory::new(
        &store,
        MemoryType::new(Pages(1024), Some(Pages(2048)), false),
    )
    .unwrap();

    #[derive(Clone, WasmerEnv)]
    pub struct Env {
        memory: Memory,
    }

    pub fn banana(
        env: &Env,
        a: u64,
        b: u64,
        c: u64,
        d: u64,
        e: u64,
        f: u64,
        g: u64,
        h: u64,
    ) -> u64 {
        println!("{:?}", (a, b, c, d, e, f, g, h));
        let view = env.memory.view::<u8>();
        let bytes = view
            .get(e as usize..(e + d) as usize)
            .unwrap()
            .into_iter()
            .map(|b| b.get())
            .collect::<Vec<u8>>();
        let input_string = std::str::from_utf8(&bytes).unwrap();
        assert_eq!(input_string, "bananapeach");
        0
    }

    pub fn mango(env: &Env, a: u64) {}

    pub fn chaenomeles(env: &Env, a: u64) -> u64 {
        0
    }

    pub fn peach(env: &Env, a: u64, b: u64) -> u64 {
        0
    }

    pub fn gas(env: &Env, a: u32) {}

    let wat = r#"
    (module
      (type (;0;) (func (param i64)))
      (type (;1;) (func (param i64) (result i64)))
      (type (;2;) (func (param i64 i64) (result i64)))
      (type (;3;) (func (param i64 i64 i64 i64 i64 i64 i64 i64) (result i64)))
      (type (;4;) (func))
      (import "env" "mango" (func (;0;) (type 0)))
      (import "env" "chaenomeles" (func (;1;) (type 1)))
      (import "env" "peach" (func (;2;) (type 2)))
      (import "env" "banana" (func (;3;) (type 3)))
      (import "env" "memory" (memory (;0;) 1024 2048))
      (func (;4;) (type 4)
        (local i32 i64)
        global.get 0
        i32.const 32
        i32.sub
        local.tee 0
        global.set 0
        local.get 0
        i32.const 8
        i32.add
        i64.const 0
        i64.store
        local.get 0
        i64.const 0
        i64.store
        i64.const 0
        call 0
        i64.const 0
        call 1
        local.set 1
        local.get 0
        i64.const 0
        i64.store offset=24
        local.get 0
        i64.const 0
        i64.store offset=16
        i64.const 0
        i64.const 0
        call 2
        local.get 1
        local.get 0
        i64.extend_i32_u
        i64.const 11
        i32.const 1048576
        i64.extend_i32_u
        i64.const 0
        i64.const 0
        local.get 0
        i32.const 16
        i32.add
        i64.extend_i32_u
        call 3
        return)
      (global (;0;) (mut i32) (i32.const 1048576))
      (global (;1;) i32 (i32.const 1048587))
      (global (;2;) i32 (i32.const 1048592))
      (global (;3;) (mut i32) (i32.const 0))
      (export "memory" (memory 0))
      (export "repro" (func 4))
      (export "__data_end" (global 1))
      (export "__heap_base" (global 2))
      (data (;0;) (i32.const 1048576) "bananapeach"))
    "#;

    let module = Module::new(&store, wat)?;
    let env = Env {
        memory: memory.clone(),
    };
    let mut exports = Exports::new();
    exports.insert("memory", memory);
    exports.insert(
        "banana",
        Function::new_native_with_env(&store, env.clone(), banana),
    );
    exports.insert(
        "peach",
        Function::new_native_with_env(&store, env.clone(), peach),
    );
    exports.insert(
        "chaenomeles",
        Function::new_native_with_env(&store, env.clone(), chaenomeles),
    );
    exports.insert(
        "mango",
        Function::new_native_with_env(&store, env.clone(), mango),
    );
    exports.insert(
        "gas",
        Function::new_native_with_env(&store, env.clone(), gas),
    );
    let mut imports = ImportObject::new();
    imports.register("env", exports);
    let instance = Instance::new(&module, &imports)?;
    instance.exports.get_function("repro")?.call(&[])?;
    Ok(())
}

/// Exhaustion of GPRs when calling a function with many floating point arguments
///
/// Note: this one is specific to Singlepass, but we want to test in all
/// available compilers.
#[compiler_test(issues)]
fn regression_gpr_exhaustion_for_calls(mut config: crate::Config) -> Result<()> {
    let store = config.store();
    let wat = r#"
        (module
          (type (;0;) (func (param f64) (result i32)))
          (type (;1;) (func (param f64 f64 f64 f64 f64 f64)))
          (func (;0;) (type 0) (param f64) (result i32)
            local.get 0
            local.get 0
            local.get 0
            local.get 0
            f64.const 0
            f64.const 0
            f64.const 0
            f64.const 0
            f64.const 0
            f64.const 0
            f64.const 0
            i32.const 0
            call_indirect (type 0)
            call_indirect (type 1)
            drop
            drop
            drop
            drop
            i32.const 0)
          (table (;0;) 1 1 funcref))
    "#;
    let module = Module::new(&store, wat)?;
    let imports: ImportObject = imports! {};
    let instance = Instance::new(&module, &imports)?;
    Ok(())
}

/// Create a large number of local (more than 0x1_0000 bytes, thats 32*16 i64 + 1)
/// to trigger an issue in the arm64 singlepass compiler
/// sequence
///   mov x17, #0x1010
///   sub xsp, xsp, x17
/// will tranform to
///   mov x17, #0x1010
///   sub xzr, xzr, x17
/// and the locals
/// on stack can get corrupted by subsequent calls if they also have locals on stack
#[compiler_test(issues)]
fn large_number_local(mut config: crate::Config) -> Result<()> {
    let mut store = config.store();
    let wat = r#"
      (module
        (func (;0;)
          (local i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64)
          i64.const 0
          i64.const 5555
          i64.add
          local.set 8
          i64.const 0
          i64.const 5555
          i64.add
          local.set 9
          i64.const 0
          i64.const 5555
          i64.add
          local.set 10
        )
        (func $large_local (export "large_local") (result i64)
          (local
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64
           i64
          )
          (local.set 15 (i64.const 1))
          (call 0)
          local.get 6
          local.get 7
          i64.add
          local.get 8
          i64.add
          local.get 9
          i64.add
          local.get 10
          i64.add
          local.get 11
          i64.add
          local.get 12
          i64.add
          local.get 13
          i64.add
          local.get 14
          i64.add
          local.get 15
          i64.add
          local.get 16
          i64.add
        )
      )
    "#;

    let module = Module::new(&store, wat)?;
    let imports: ImportObject = imports! {};
    let instance = Instance::new(&module, &imports)?;
    let result = instance
        .exports
        .get_function("large_local")?
        .call(&[])
        .unwrap();
    assert_eq!(&Value::I64(1 as i64), result.get(0).unwrap());
    Ok(())
}

/// `Instance::reset` must keep the static-memory guard region intact.
///
/// Compiled code for static memories carries no bounds checks: it relies on
/// the `bound + guard` reservation made at instantiation. Shrinking the memory
/// back to its minimum on reset used to replace that reservation with a
/// `minimum + guard` one, moving the base and leaving every address above the
/// new mapping unprotected.
#[compiler_test(issues)]
fn memory_guard_survives_reset(config: crate::Config) -> Result<()> {
    let store = config.store();
    let wat = r#"
    (module
        (memory (export "memory") 1 100)
        (data (i32.const 0) "\01\02\03\04")
        (func (export "grow") (param i32) (result i32)
            (memory.grow (local.get 0)))
        (func (export "load") (param i32) (result i32)
            (i32.load (local.get 0)))
        (func (export "store") (param i32 i32)
            (i32.store (local.get 0) (local.get 1))))
    "#;
    let module = Module::new(&store, wat)?;
    let instance = Instance::new(&module, &imports! {})?;
    let memory = instance.exports.get_memory("memory")?;
    let grow = instance.exports.get_native_function::<i32, i32>("grow")?;
    let load = instance.exports.get_native_function::<i32, i32>("load")?;
    let store_word = instance
        .exports
        .get_native_function::<(i32, i32), ()>("store")?;

    let page = WASM_PAGE_SIZE as i32;
    let base = memory.data_ptr();

    // Grow, then dirty a page in the grown tail.
    assert_eq!(grow.call(3)?, 1);
    store_word.call(3 * page, 0x5eed_5eed)?;
    assert_eq!(load.call(3 * page)?, 0x5eed_5eed);

    instance.reset().map_err(anyhow::Error::msg)?;

    assert_eq!(memory.size(), Pages(1));
    assert_eq!(
        memory.data_ptr(),
        base,
        "reset must not move a static memory"
    );
    assert_eq!(load.call(0)?, 0x0403_0201, "data segments are re-applied");

    // The previously grown page is out of bounds again, and so is everything
    // up to the end of the 4 GiB static bound.
    assert_oob(load.call(3 * page));
    assert_oob(load.call(0x4000_0000));
    assert_oob(load.call(0xC000_0000_u32 as i32));
    assert_oob(load.call(-4));

    // Growing again reuses the same mapping and yields zeroed pages.
    assert_eq!(grow.call(3)?, 1);
    assert_eq!(memory.data_ptr(), base);
    assert_eq!(load.call(3 * page)?, 0);

    return Ok(());

    fn assert_oob(result: std::result::Result<i32, RuntimeError>) {
        let err = result.expect_err("access past the current memory size must trap");
        let trap = err
            .to_trap()
            .expect("must be a wasm trap, not a host error");
        assert_eq!(trap.message(), "out of bounds memory access");
    }
}
