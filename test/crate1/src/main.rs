mod mod1;

async fn main() {
    let x = result().await;
    other().await;
}

async fn result() -> Result<(), MyError> {
    Ok(())
}

async fn other() {

}

struct MyError;