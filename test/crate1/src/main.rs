fn main() -> Result<(), MyError> {
    other()
}

async fn result() -> Result<(), MyError> {
    Ok(())
}

async fn propagate() -> Result<(), MyError> {
    result().await?;

    result().await;

    return result().await;
}

fn other() -> Result<(), MyError> {
    main()
}

#[derive(Debug)]
struct MyError;