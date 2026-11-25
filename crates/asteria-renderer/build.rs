//! Mozc の protocol/*.proto と ipc/ipc.proto(BSD-3、Asteria のパッチを当てた木から写したもの)から型を作る。

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto");
    let fds = protox::compile(
        ["protocol/renderer_command.proto", "ipc/ipc.proto"],
        ["proto"],
    )?;
    prost_build::Config::new().compile_fds(fds)?;
    Ok(())
}
