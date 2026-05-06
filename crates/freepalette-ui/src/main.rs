fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_target(false).init();
    println!("{}", freepalette_ui::status());
    Ok(())
}
