use std::path::Path;

use rotala::source::hyperliquid::get_hyperliquid_l2;


pub fn main() {

    let path = Path::new("/tmp/SOL");

    get_hyperliquid_l2(path);

}
