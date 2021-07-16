use itertools::Itertools;

pub trait DefinedUniverse {
    fn get_assets(&self) -> &Vec<String>;
}

pub struct Universe {
    assets: Vec<String>,
}

pub struct StaticUniverse {
    assets: Vec<String>,
}

impl DefinedUniverse for StaticUniverse {
    fn get_assets(&self) -> &Vec<String> {
        &self.assets
    }
}

impl StaticUniverse {
    pub fn new(assets: Vec<&str>) -> StaticUniverse {
        let to_string = assets.iter().map(|v| String::from(*v)).collect_vec();
        StaticUniverse { assets: to_string }
    }
}
