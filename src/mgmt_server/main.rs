fn main() -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(actual_main())
}

/// tokio::main macro doesn't work if there are multiple binaries
async fn actual_main() -> Result<(), Box<dyn std::error::Error>> {
    println!("hi");
    Ok(())
}
