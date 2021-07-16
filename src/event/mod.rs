use std::cell::RefCell;
use std::rc::Rc;

use super::types;

pub trait Update {
    fn update(&mut self, event: &types::Event);
}

pub struct EventEmitter {
    observers: Vec<Rc<RefCell<dyn Update>>>,
}

impl EventEmitter {
    pub fn emit(&mut self, event: &types::Event) {
        for o in &mut self.observers {
            o.borrow_mut().update(event)
        }
    }

    pub fn subscribe(&mut self, observer: Rc<RefCell<dyn Update>>) {
        self.observers.push(observer);
    }

    pub fn new() -> EventEmitter {
        let ev = Vec::new();
        EventEmitter { observers: ev }
    }
}
