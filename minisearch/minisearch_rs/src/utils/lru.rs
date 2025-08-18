use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::rc::{Rc, Weak};

#[derive(Clone, Debug)]
struct Value<K, T> {
    key: K,
    val: T,
}

#[derive(Debug)]
struct Node<T> {
    val: T,
    prev: Option<Rc<RefCell<Node<T>>>>,
    next: Option<Weak<RefCell<Node<T>>>>,
}

#[derive(Debug)]
struct List<T: Copy> {
    size: u32,
    head: Option<Rc<RefCell<Node<T>>>>,
    tail: Option<Rc<RefCell<Node<T>>>>,
}

#[derive(Debug)]
pub struct LRUCache<K: Eq + Hash + Copy, T: Copy> {
    capacity: u32,
    map: HashMap<K, Rc<RefCell<Node<Value<K, T>>>>>,
    list: List<Value<K, T>>,
}

impl<K: Copy, T: Copy> Copy for Value<K, T> {}

impl<T> Node<T> {
    fn new(val: T) -> Node<T> {
        Node {
            val: val,
            prev: None,
            next: None,
        }
    }
}

impl<T: Copy> List<T> {
    fn new() -> List<T> {
        List {
            size: 0,
            head: None,
            tail: None,
        }
    }

    fn push(&mut self, val: T) -> Rc<RefCell<Node<T>>> {
        // push to the front of the linked list
        let node = Rc::new(RefCell::new(Node::new(val)));

        // update prev link
        match self.head.take() {
            Some(head) => {
                head.borrow_mut().prev = Some(Rc::clone(&node));
                node.borrow_mut().next = Some(Rc::downgrade(&head));
            }
            None => {
                self.tail = Some(Rc::clone(&node));
            }
        }

        self.head = Some(Rc::clone(&node));
        self.size += 1;

        node
    }

    fn pop(&mut self) -> Option<T> {
        // removes last node from the back

        match self.tail.take() {
            Some(tail) => {
                let mut tail = tail.borrow_mut();
                match tail.prev.take() {
                    Some(prev) => {
                        prev.borrow_mut().next = None;
                        self.tail = Some(prev);
                    }
                    None => {
                        self.head.take();
                    }
                };

                self.size -= 1;
                Some(tail.val)
            }
            _ => None,
        }
    }

    fn move_front(&mut self, node: Rc<RefCell<Node<T>>>) {
        let mut n = node.borrow_mut();
        let mut prev = n.prev.take();

        // change linking of neighbour nodes
        match prev {
            Some(ref mut prev) => {
                if let Some(ref next) = n.next {
                    prev.borrow_mut().next.replace(Weak::clone(next));
                } else {
                    prev.borrow_mut().next.take();
                }
            }
            // if value is first, do nothing
            None => return,
        }

        let mut next = n.next.take();
        match next {
            Some(ref mut next) => {
                if let Some(ref mut next) = next.upgrade() {
                    if let Some(ref prev) = prev {
                        next.borrow_mut().prev.replace(Rc::clone(prev));
                    }
                }
            }
            None => {
                // set tail
                self.tail = prev
            }
        }

        // add node to the front of the list
        match self.head.take() {
            Some(head) => {
                n.next.replace(Rc::downgrade(&head));
                head.borrow_mut().prev.replace(Rc::clone(&node));
            }
            None => (),
        }

        self.head.replace(Rc::clone(&node));
    }
}

impl<K: Eq + Hash + Copy, T: Copy> LRUCache<K, T> {
    pub fn new(capacity: u32) -> Self {
        Self {
            capacity: capacity,
            map: HashMap::new(),
            list: List::new(),
        }
    }

    pub fn add(&mut self, key: K, val: T) {
        if self.map.contains_key(&key) {
            match self.map.get(&key) {
                Some(node) => {
                    // move to front
                    self.list.move_front(Rc::clone(node));
                    // update value
                    let mut node = node.borrow_mut();
                    node.val.val = val;
                }
                None => (),
            }
        } else {
            let node = self.list.push(Value { key: key, val: val });
            self.map.insert(key, node);
        }

        if self.list.size > self.capacity {
            match self.list.pop() {
                Some(val) => {
                    self.map.remove(&val.key);
                }
                None => (),
            }
        }
    }

    pub fn get(&mut self, key: K) -> Option<T> {
        match self.map.get(&key) {
            Some(node) => {
                // move node to front
                self.list.move_front(Rc::clone(node));
                // return value
                let node = node.borrow_mut();
                Some(node.val.val)
            }
            None => None,
        }
    }
}
