fn main() {
    hello();
    test();
    hello();
}

fn hello() {
    println!("Hello!");
    main();
}

fn test() {
    hello();
    main();
}

fn unreachable() {

}
