#![forbid(unsafe_code)]

use std::fmt;
use std::io::{self, Read, Seek, SeekFrom, Write};

#[derive(Clone, Copy, Debug, PartialEq, Eq)] enum Mode { Trim, Reverse, ChannelMap, Normalize }
const MODE: Mode = Mode::Trim;

#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Endianness { Little, Big }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum IntegerEncoding { Signed, Unsigned }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub channels: u16,
    pub bits_per_sample: u16,
    pub input_endianness: Endianness,
    pub output_endianness: Endianness,
    pub input_encoding: IntegerEncoding,
    pub output_encoding: IntegerEncoding,
    pub start_frame: u64,
    pub frame_count: Option<u64>,
    pub channel_map: Vec<u16>,
    pub buffer_size: usize,
    pub max_channels: u16,
}
impl Default for Options {
    fn default() -> Self {
        match MODE {
            Mode::Trim => Self { channels:1,bits_per_sample:16,input_endianness:Endianness::Little,output_endianness:Endianness::Little,input_encoding:IntegerEncoding::Signed,output_encoding:IntegerEncoding::Signed,start_frame:0,frame_count:None,channel_map:vec![0],buffer_size:65_536,max_channels:256 },
            Mode::Reverse => Self { channels:1,bits_per_sample:16,input_endianness:Endianness::Little,output_endianness:Endianness::Little,input_encoding:IntegerEncoding::Signed,output_encoding:IntegerEncoding::Signed,start_frame:0,frame_count:None,channel_map:vec![0],buffer_size:65_536,max_channels:256 },
            Mode::ChannelMap => Self { channels:2,bits_per_sample:16,input_endianness:Endianness::Little,output_endianness:Endianness::Little,input_encoding:IntegerEncoding::Signed,output_encoding:IntegerEncoding::Signed,start_frame:0,frame_count:None,channel_map:vec![1,0],buffer_size:65_536,max_channels:256 },
            Mode::Normalize => Self { channels:1,bits_per_sample:16,input_endianness:Endianness::Little,output_endianness:Endianness::Big,input_encoding:IntegerEncoding::Signed,output_encoding:IntegerEncoding::Signed,start_frame:0,frame_count:None,channel_map:vec![0],buffer_size:65_536,max_channels:256 },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report { pub input_bytes:u64,pub output_bytes:u64,pub input_frames:u64,pub output_frames:u64,pub input_channels:u16,pub output_channels:u16,pub bits_per_sample:u16,pub peak_working_memory_bytes:u64,pub warnings:Vec<String> }
#[derive(Debug)] pub enum Error { Io(io::Error), Invalid(&'static str), Overflow }
impl fmt::Display for Error { fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{match self{Self::Io(e)=>e.fmt(f),Self::Invalid(m)=>f.write_str(m),Self::Overflow=>f.write_str("size arithmetic overflow")}} }
impl std::error::Error for Error { fn source(&self)->Option<&(dyn std::error::Error+'static)>{match self{Self::Io(e)=>Some(e),_=>None}} }
impl From<io::Error> for Error { fn from(value:io::Error)->Self{Self::Io(value)} }

fn validate(options:&Options)->Result<(usize,usize),Error>{if options.channels==0||options.channels>options.max_channels{return Err(Error::Invalid("channels must be between 1 and max_channels"));}if !matches!(options.bits_per_sample,8|16|24|32){return Err(Error::Invalid("bits_per_sample must be 8, 16, 24 or 32"));}if !(256..=16*1024*1024).contains(&options.buffer_size){return Err(Error::Invalid("buffer_size must be between 256 bytes and 16 MiB"));}let width=usize::from(options.bits_per_sample/8);let frame=usize::from(options.channels).checked_mul(width).ok_or(Error::Overflow)?;if MODE==Mode::ChannelMap{if options.channel_map.is_empty()||options.channel_map.len()>usize::from(options.max_channels){return Err(Error::Invalid("channel_map must contain 1 through max_channels entries"));}if options.channel_map.iter().any(|c|*c>=options.channels){return Err(Error::Invalid("channel_map references a missing input channel"));}}Ok((width,frame))}
fn extent<R:Seek>(input:&mut R,frame:usize)->Result<(u64,u64,u64),Error>{let start=input.stream_position()?;let end=input.seek(SeekFrom::End(0))?;if end<start{return Err(Error::Overflow);}let bytes=end-start;if bytes%frame as u64!=0{return Err(Error::Invalid("raw PCM input is not frame aligned"));}input.seek(SeekFrom::Start(start))?;Ok((start,bytes,bytes/frame as u64))}
fn copy_exact<R:Read,W:Write>(input:&mut R,output:&mut W,mut bytes:u64,buffer_size:usize)->Result<(),Error>{let mut buffer=vec![0u8;buffer_size];while bytes!=0{let take=bytes.min(buffer.len()as u64)as usize;input.read_exact(&mut buffer[..take])?;output.write_all(&buffer[..take])?;bytes-=take as u64;}Ok(())}
fn trim<R:Read+Seek,W:Write>(input:&mut R,output:&mut W,o:&Options,frame:usize,start:u64,frames:u64)->Result<(u64,u64),Error>{if o.start_frame>frames{return Err(Error::Invalid("start_frame exceeds input frame count"));}let available=frames-o.start_frame;let count=o.frame_count.unwrap_or(available);if count>available{return Err(Error::Invalid("requested frame_count exceeds input"));}let offset=o.start_frame.checked_mul(frame as u64).and_then(|v|v.checked_add(start)).ok_or(Error::Overflow)?;input.seek(SeekFrom::Start(offset))?;let bytes=count.checked_mul(frame as u64).ok_or(Error::Overflow)?;let length=o.buffer_size-(o.buffer_size%frame);copy_exact(input,output,bytes,length.max(frame))?;Ok((count,bytes))}
fn reverse<R:Read+Seek,W:Write>(input:&mut R,output:&mut W,o:&Options,frame:usize,start:u64,frames:u64)->Result<(u64,u64),Error>{let capacity=(o.buffer_size/frame).max(1);let mut buffer=vec![0u8;capacity*frame];let mut remaining=frames;while remaining!=0{let take=remaining.min(capacity as u64);let first=remaining-take;let offset=start.checked_add(first.checked_mul(frame as u64).ok_or(Error::Overflow)?).ok_or(Error::Overflow)?;input.seek(SeekFrom::Start(offset))?;let bytes=take as usize*frame;input.read_exact(&mut buffer[..bytes])?;for item in buffer[..bytes].chunks_exact(frame).rev(){output.write_all(item)?;}remaining=first;}Ok((frames,frames.checked_mul(frame as u64).ok_or(Error::Overflow)?))}
fn channel_map<R:Read+Seek,W:Write>(input:&mut R,output:&mut W,o:&Options,width:usize,frame:usize,frames:u64)->Result<(u64,u64),Error>{let output_frame=o.channel_map.len().checked_mul(width).ok_or(Error::Overflow)?;let count=(o.buffer_size/frame).max(1);let mut source=vec![0u8;count*frame];let mut target=Vec::with_capacity(count*output_frame);let mut remaining=frames;while remaining!=0{let take=remaining.min(count as u64)as usize;let bytes=take*frame;input.read_exact(&mut source[..bytes])?;target.clear();for input_frame in source[..bytes].chunks_exact(frame){for channel in &o.channel_map{let begin=usize::from(*channel)*width;target.extend_from_slice(&input_frame[begin..begin+width]);}}output.write_all(&target)?;remaining-=take as u64;}let output_bytes=frames.checked_mul(output_frame as u64).ok_or(Error::Overflow)?;Ok((frames,output_bytes))}
fn normalize_sample(sample:&mut[u8],o:&Options){if sample.len()>1&&o.input_endianness!=o.output_endianness{sample.reverse();}if o.input_encoding!=o.output_encoding{let index=if sample.len()==1||o.output_endianness==Endianness::Big{0}else{sample.len()-1};sample[index]^=0x80;}}
fn normalize<R:Read+Seek,W:Write>(input:&mut R,output:&mut W,o:&Options,width:usize,frame:usize,frames:u64)->Result<(u64,u64),Error>{let length=(o.buffer_size-(o.buffer_size%frame)).max(frame);let mut buffer=vec![0u8;length];let mut remaining=frames.checked_mul(frame as u64).ok_or(Error::Overflow)?;while remaining!=0{let take=remaining.min(buffer.len()as u64)as usize;input.read_exact(&mut buffer[..take])?;for sample in buffer[..take].chunks_exact_mut(width){normalize_sample(sample,o);}output.write_all(&buffer[..take])?;remaining-=take as u64;}Ok((frames,frames*frame as u64))}

/// Apply the declared raw PCM operation using explicit parameter-owned semantics.
pub fn convert<R:Read+Seek,W:Write>(input:&mut R,output:&mut W,options:&Options)->Result<Report,Error>{let(width,frame)=validate(options)?;let(start,input_bytes,input_frames)=extent(input,frame)?;let(output_frames,output_bytes)=match MODE{Mode::Trim=>trim(input,output,options,frame,start,input_frames)?,Mode::Reverse=>reverse(input,output,options,frame,start,input_frames)?,Mode::ChannelMap=>channel_map(input,output,options,width,frame,input_frames)?,Mode::Normalize=>normalize(input,output,options,width,frame,input_frames)?};Ok(Report{input_bytes,output_bytes,input_frames,output_frames,input_channels:options.channels,output_channels:if MODE==Mode::ChannelMap{options.channel_map.len()as u16}else{options.channels},bits_per_sample:options.bits_per_sample,peak_working_memory_bytes:options.buffer_size as u64*2,warnings:Vec::new()})}

#[doc(hidden)] pub fn conformance_fixture()->Vec<u8>{match MODE{Mode::ChannelMap=>vec![1,0,2,0,3,0,4,0],_=>vec![1,0,2,0,3,0]}}

#[cfg(test)]mod tests{use super::*;use std::io::Cursor;
#[test]fn defaults_are_runnable_and_transform(){let source=conformance_fixture();let mut output=Vec::new();let report=convert(&mut Cursor::new(source.clone()),&mut output,&Options::default()).unwrap();assert!(report.output_frames>0);match MODE{Mode::Trim=>assert_eq!(output,source),Mode::Reverse=>assert_eq!(output,[3,0,2,0,1,0]),Mode::ChannelMap=>assert_eq!(output,[2,0,1,0,4,0,3,0]),Mode::Normalize=>assert_eq!(output,[0,1,0,2,0,3])}}
#[test]fn rejects_partial_frames(){let mut source=conformance_fixture();source.push(9);assert!(convert(&mut Cursor::new(source),&mut Vec::new(),&Options::default()).is_err());}
#[test]fn validates_buffer_bounds(){let mut o=Options::default();o.buffer_size=1;assert!(convert(&mut Cursor::new(conformance_fixture()),&mut Vec::new(),&o).is_err());}
#[test]fn custom_semantics_are_honored(){let mut o=Options::default();let mut output=Vec::new();match MODE{Mode::Trim=>{o.start_frame=1;o.frame_count=Some(1);convert(&mut Cursor::new(vec![1,0,2,0,3,0]),&mut output,&o).unwrap();assert_eq!(output,[2,0]);},Mode::Reverse=>{o.channels=2;convert(&mut Cursor::new(vec![1,0,2,0,3,0,4,0]),&mut output,&o).unwrap();assert_eq!(output,[3,0,4,0,1,0,2,0]);},Mode::ChannelMap=>{o.channel_map=vec![0,0];convert(&mut Cursor::new(vec![1,0,2,0]),&mut output,&o).unwrap();assert_eq!(output,[1,0,1,0]);},Mode::Normalize=>{o.bits_per_sample=8;o.input_encoding=IntegerEncoding::Signed;o.output_encoding=IntegerEncoding::Unsigned;convert(&mut Cursor::new(vec![0x80,0,0x7f]),&mut output,&o).unwrap();assert_eq!(output,[0,0x80,0xff]);}}}
}
