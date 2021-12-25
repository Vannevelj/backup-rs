use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Options {
    path: String
}

fn main() {
    let args = Options::from_args();
    println!("{:?}", args);
}
