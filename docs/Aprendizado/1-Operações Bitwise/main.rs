fn main(){
    let x: u8 = 1;
    let y: u8 = 2;
    print!("x = {}\ny = {}\n", x,y);
    println!("x & y = {}", x & y);
    println!("x | y = {}", x | y);
    println!("~x = {}", !x);
    println!("x ^ y = {}", x ^ y);
    println!("x << 2 = {}", x << 2);
    println!("x >> 2 = {}", x >> 2);
}