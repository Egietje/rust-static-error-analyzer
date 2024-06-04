mod mod1;

fn main() {
    async {
        let x = result().await;
        other().await;
    };
}

async fn result() -> Result<(), MyError> {
    Ok(())
}

async fn other() {

}

struct MyError;