mod test;

use test::test as help;

fn main() {
    hello();
    test();
    hello();
}

fn hello() {
    println!("Hello!");
    main();
    help();
}

fn test() {
    test::test();
    hello();
    main();
}

fn unreachable() {}
