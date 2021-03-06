use std::io::Read;

use crate::code_pair_put_back::CodePairPutBack;
use crate::objects::Object;

pub(crate) struct ObjectIter<'a, I: 'a + Read> {
    pub iter: &'a mut CodePairPutBack<I>,
}

impl<'a, I: 'a + Read> Iterator for ObjectIter<'a, I> {
    type Item = Object;

    fn next(&mut self) -> Option<Object> {
        match Object::read(self.iter) {
            Ok(Some(o)) => Some(o),
            Ok(None) | Err(_) => None,
        }
    }
}
