fn main() {
    async {
        let x = propagate().await;
        other().await;
    };
}

async fn result() -> Result<(), MyError> {
    Ok(())
}

async fn propagate() -> Result<(), MyError> {
    result().await?;

    result().await;

    return result().await;
}

async fn other() {

}

struct MyError;