FROM arm64v8/golang:1.20.7 as builder

RUN apt-get update && apt-get install -y \
    patchelf \
    build-essential

COPY ./capi_shim /capi_shim
WORKDIR /capi_shim
RUN go build -buildmode=c-shared -o libwasmer_linux_arm64_shim.so .
RUN patchelf --set-soname libwasmer_linux_arm64_shim.so libwasmer_linux_arm64_shim.so

FROM arm64v8/ubuntu:22.04

COPY --from=builder /capi_shim/libwasmer_linux_arm64_shim.so /data/libwasmer_linux_arm64_shim.so