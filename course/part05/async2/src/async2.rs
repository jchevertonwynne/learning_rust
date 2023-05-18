// making an executor

fn main() -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    // let rt = tokio::runtime::Builder::new_current_thread()
    //     .enable_all()
    //     .build()?;
    rt.block_on(hello_world());

    Ok(())
}

async fn hello_world() {
    println!("hello world!");
}
