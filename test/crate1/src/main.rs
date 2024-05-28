mod mod1;

fn main() {
    test(&mut Test);
}

fn fn1() -> Result<(), ()> {
    Err(())
}

fn test(test: &mut Test) {
    // potentially calls test.ja()
}

struct Test;

impl Test {
    fn ja(self) {
        fn1();
    }
}
