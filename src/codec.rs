use bytes::{Buf, BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use tokio_util::codec::{Decoder, Encoder};

use crate::AppendEntries;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    AppendEntries(AppendEntries)
}

impl Message {
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(self).unwrap();
        bytes.push(b'\r');
        bytes.push(b'\n');
        bytes
    }
}

#[derive(Debug)]
pub(crate) struct MessageCodec;

impl MessageCodec {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Encoder<Message> for MessageCodec {
    type Error = anyhow::Error;
    fn encode(&mut self, item: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.put_slice(&item.serialize());
        Ok(())
    }
}

impl Decoder for MessageCodec {
    type Item = Message;
    type Error = anyhow::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 2 {
            return Ok(None);
        }
        let end = src.len() - 1;

        for i in 0..end {
            if src[i] == b'\r' && src[i + 1] == b'\n' {
                let x = serde_json::from_slice(&src[..i]).map_err(|e| e.into());
                src.advance(i + 2);
                return x;
            }
        }

        Ok(None)
    }
}


