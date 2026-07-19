#![forbid(unsafe_code)]

use std::fmt;
use std::io::{self, Read, Seek, SeekFrom, Write};

const PCM_GUID: [u8; 16] = [
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00,
    0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
];
const W64_RIFF: [u8; 16] = [0x72,0x69,0x66,0x66,0x2e,0x91,0xcf,0x11,0xa5,0xd6,0x28,0xdb,0x04,0xc1,0x00,0x00];
const W64_WAVE: [u8; 16] = [0x77,0x61,0x76,0x65,0xf3,0xac,0xd3,0x11,0x8c,0xd1,0x00,0xc0,0x4f,0x8e,0xdb,0x8a];
const W64_FMT: [u8; 16] = [0x66,0x6d,0x74,0x20,0xf3,0xac,0xd3,0x11,0x8c,0xd1,0x00,0xc0,0x4f,0x8e,0xdb,0x8a];
const W64_DATA: [u8; 16] = [0x64,0x61,0x74,0x61,0xf3,0xac,0xd3,0x11,0x8c,0xd1,0x00,0xc0,0x4f,0x8e,0xdb,0x8a];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Profile { Wav, Aiff, Caf, Au, Rf64, Bw64, Wave64, Bwf }
const SOURCE: Profile = Profile::Caf;
const TARGET: Profile = Profile::Wave64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Endian { Little, Big }
#[derive(Clone, Copy, Debug)]
struct Storage { endian: Endian, eight_bit_signed: bool }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub strict_header_consistency: bool,
    pub buffer_size: usize,
    pub max_channels: u16,
}
impl Default for Options {
    fn default() -> Self { Self { strict_header_consistency: true, buffer_size: 65_536, max_channels: 256 } }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub input_bytes: u64,
    pub output_bytes: u64,
    pub audio_bytes: u64,
    pub channels: u16,
    pub sample_rate: u32,
    pub container_bits_per_sample: u16,
    pub valid_bits_per_sample: u16,
    pub sample_frames: u64,
    pub peak_working_memory_bytes: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum Error { Io(io::Error), Invalid(&'static str), Unsupported(&'static str), SizeLimit, Overflow }
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { match self {
        Self::Io(error) => error.fmt(f), Self::Invalid(message) | Self::Unsupported(message) => f.write_str(message),
        Self::SizeLimit => f.write_str("target container size limit exceeded"), Self::Overflow => f.write_str("size arithmetic overflow"),
    }}
}
impl std::error::Error for Error { fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { match self { Self::Io(error) => Some(error), _ => None } } }
impl From<io::Error> for Error { fn from(value: io::Error) -> Self { Self::Io(value) } }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Format { channels: u16, rate: u32, container_bits: u16, valid_bits: u16, block_align: u16 }
#[derive(Clone, Copy, Debug)] struct Segment { offset: u64, size: u64 }
#[derive(Debug)] struct Parsed { input_bytes: u64, format: Format, storage: Storage, segments: Vec<Segment>, audio_bytes: u64, warnings: Vec<String> }
struct Plan { prefix: Vec<u8>, padding: usize }

fn le16(bytes: &[u8]) -> u16 { u16::from_le_bytes([bytes[0],bytes[1]]) }
fn le32(bytes: &[u8]) -> u32 { u32::from_le_bytes([bytes[0],bytes[1],bytes[2],bytes[3]]) }
fn le64(bytes: &[u8]) -> u64 { u64::from_le_bytes(bytes.try_into().expect("eight bytes")) }
fn be16(bytes: &[u8]) -> u16 { u16::from_be_bytes([bytes[0],bytes[1]]) }
fn be32(bytes: &[u8]) -> u32 { u32::from_be_bytes(bytes.try_into().expect("four bytes")) }
fn be64(bytes: &[u8]) -> u64 { u64::from_be_bytes(bytes.try_into().expect("eight bytes")) }
fn align(value: u64, alignment: u64) -> Result<u64, Error> { value.checked_add(alignment - 1).map(|v| v / alignment * alignment).ok_or(Error::Overflow) }

fn storage(profile: Profile) -> Storage { match profile {
    Profile::Aiff | Profile::Au => Storage { endian: Endian::Big, eight_bit_signed: true },
    Profile::Caf => Storage { endian: Endian::Little, eight_bit_signed: true },
    _ => Storage { endian: Endian::Little, eight_bit_signed: false },
} }

fn parse_wave_format(bytes: &[u8], options: &Options) -> Result<Format, Error> {
    if bytes.len() < 16 { return Err(Error::Invalid("fmt chunk is shorter than 16 bytes")); }
    let tag=le16(&bytes[0..2]); let channels=le16(&bytes[2..4]); let rate=le32(&bytes[4..8]); let byte_rate=le32(&bytes[8..12]); let block=le16(&bytes[12..14]); let bits=le16(&bytes[14..16]);
    if channels==0 || channels>options.max_channels { return Err(Error::Unsupported("channel count is unsupported")); }
    if rate==0 || !matches!(bits,8|16|24|32) { return Err(Error::Unsupported("sample rate or PCM width is unsupported")); }
    let valid=match tag { 1=>bits, 0xfffe=>{ if bytes.len()<40 || le16(&bytes[16..18])<22 || bytes[24..40]!=PCM_GUID { return Err(Error::Unsupported("WAVE extensible subtype is not integer PCM")); } let value=le16(&bytes[18..20]); if value==0 || value>bits{return Err(Error::Invalid("valid bits are invalid"));} value }, _=>return Err(Error::Unsupported("WAVE format is not integer PCM")) };
    let expected=u32::from(channels)*u32::from(bits/8); if u32::from(block)!=expected { return Err(Error::Invalid("PCM block alignment is inconsistent")); }
    if options.strict_header_consistency && byte_rate!=rate.checked_mul(expected).ok_or(Error::Overflow)? { return Err(Error::Invalid("PCM byte rate is inconsistent")); }
    Ok(Format{channels,rate,container_bits:bits,valid_bits:valid,block_align:block})
}

fn parse_riff<R: Read+Seek>(input:&mut R, profile:Profile, options:&Options)->Result<Parsed,Error>{
    let start=input.stream_position()?;let physical=input.seek(SeekFrom::End(0))?;input.seek(SeekFrom::Start(start))?;let mut head=[0u8;12];input.read_exact(&mut head)?;
    let expected=match profile{Profile::Rf64=>b"RF64",Profile::Bw64=>b"BW64",_=>b"RIFF"};if &head[0..4]!=expected||&head[8..12]!=b"WAVE"{return Err(Error::Invalid("container is not the declared WAVE profile"));}
    let extended=matches!(profile,Profile::Rf64|Profile::Bw64);let mut scan_end=if extended{physical}else{start.checked_add(8+u64::from(le32(&head[4..8]))).ok_or(Error::Overflow)?};if scan_end>physical{return Err(Error::Invalid("RIFF size extends past EOF"));}if options.strict_header_consistency&&!extended&&scan_end!=physical{return Err(Error::Invalid("RIFF size does not match input length"));}
    let mut position=start+12;let mut format=None;let mut segments=Vec::new();let mut ds64_data=None;let mut ds64_riff=None;let mut bext=false;let mut warnings=Vec::new();
    while position+8<=scan_end{input.seek(SeekFrom::Start(position))?;let mut h=[0u8;8];input.read_exact(&mut h)?;let id=&h[0..4];let declared=le32(&h[4..8]);let payload=position+8;
        let size=if id==b"data"&&declared==u32::MAX{ds64_data.ok_or(Error::Invalid("extended data precedes ds64"))?}else{u64::from(declared)};let next=payload.checked_add(size).and_then(|v|v.checked_add(size&1)).ok_or(Error::Overflow)?;if next>scan_end{return Err(Error::Invalid("RIFF chunk extends past boundary"));}
        if id==b"ds64"{if size<28{return Err(Error::Invalid("ds64 chunk is incomplete"));}let mut b=[0u8;28];input.read_exact(&mut b)?;ds64_riff=Some(le64(&b[0..8]));ds64_data=Some(le64(&b[8..16]));}
        else if id==b"fmt "{if size>4096{return Err(Error::Unsupported("fmt chunk is too large"));}let mut b=vec![0u8;size as usize];input.read_exact(&mut b)?;format=Some(parse_wave_format(&b,options)?);}
        else if id==b"data"{segments.push(Segment{offset:payload,size});}
        else if id==b"bext"{bext=true;}
        position=next;
    }
    if extended{let declared=ds64_riff.ok_or(Error::Invalid("extended WAVE is missing ds64"))?;let end=start.checked_add(8+declared).ok_or(Error::Overflow)?;if end>physical{return Err(Error::Invalid("ds64 RIFF size extends past EOF"));}if options.strict_header_consistency&&end!=physical{return Err(Error::Invalid("ds64 RIFF size does not match input length"));}scan_end=end;let _=scan_end;}
    if profile==Profile::Bwf&&!bext{return Err(Error::Invalid("BWF input has no bext chunk"));}if profile==Profile::Wav&&bext{warnings.push("BWF bext metadata was not retained by plain WAVE output".into());}
    finish(physical-start,format.ok_or(Error::Invalid("missing fmt chunk"))?,storage(profile),segments,warnings)
}

fn parse_au<R:Read+Seek>(input:&mut R,options:&Options)->Result<Parsed,Error>{
    let start=input.stream_position()?;let physical=input.seek(SeekFrom::End(0))?;input.seek(SeekFrom::Start(start))?;let mut h=[0u8;24];input.read_exact(&mut h)?;if &h[0..4]!=b".snd"{return Err(Error::Invalid("input is not Sun AU/SND"));}
    let relative=u64::from(be32(&h[4..8]));if relative<24{return Err(Error::Invalid("AU data offset is too small"));}let offset=start.checked_add(relative).ok_or(Error::Overflow)?;if offset>physical{return Err(Error::Invalid("AU data offset exceeds EOF"));}
    let declared=be32(&h[8..12]);let size=if declared==u32::MAX{physical-offset}else{u64::from(declared)};if offset+size>physical{return Err(Error::Invalid("AU data size exceeds EOF"));}if options.strict_header_consistency&&offset+size!=physical{return Err(Error::Invalid("AU declared data does not consume the input"));}
    let bits=match be32(&h[12..16]){2=>8,3=>16,4=>24,5=>32,_=>return Err(Error::Unsupported("AU encoding is not linear integer PCM"))};let rate=be32(&h[16..20]);let channels=u16::try_from(be32(&h[20..24])).map_err(|_|Error::Unsupported("AU channel count is too large"))?;if channels==0||channels>options.max_channels||rate==0{return Err(Error::Unsupported("AU channels or rate are unsupported"));}let block=channels.checked_mul(bits/8).ok_or(Error::Overflow)?;
    finish(physical-start,Format{channels,rate,container_bits:bits,valid_bits:bits,block_align:block},storage(Profile::Au),vec![Segment{offset,size}],Vec::new())
}

fn decode_aiff_rate(bytes:&[u8])->Result<u32,Error>{
    if bytes.len()!=10{return Err(Error::Invalid("AIFF sample rate is not an 80-bit extended value"));}
    let raw=be16(&bytes[0..2]);if raw&0x8000!=0{return Err(Error::Unsupported("negative AIFF sample rate"));}
    let exponent=raw&0x7fff;if exponent==0||exponent==0x7fff{return Err(Error::Unsupported("non-finite, denormal or zero AIFF sample rate"));}
    let mantissa=be64(&bytes[2..10]);if mantissa&(1u64<<63)==0{return Err(Error::Invalid("AIFF sample rate has no integer bit"));}
    let shift=i32::from(exponent)-16_383-63;
    let value=if shift>=0{mantissa.checked_shl(shift as u32).ok_or(Error::Unsupported("AIFF sample rate is too large"))?}else{let right=(-shift)as u32;if right>=64{return Err(Error::Unsupported("fractional AIFF sample rate is unsupported"));}let mask=if right==0{0}else{(1u64<<right)-1};if mantissa&mask!=0{return Err(Error::Unsupported("fractional AIFF sample rate is unsupported"));}mantissa>>right};
    u32::try_from(value).ok().filter(|value|*value!=0).ok_or(Error::Unsupported("AIFF sample rate is outside the integer range"))
}

fn parse_aiff<R:Read+Seek>(input:&mut R,options:&Options)->Result<Parsed,Error>{
    let start=input.stream_position()?;let physical=input.seek(SeekFrom::End(0))?;input.seek(SeekFrom::Start(start))?;let mut h=[0u8;12];input.read_exact(&mut h)?;
    if &h[0..4]!=b"FORM"||&h[8..12]!=b"AIFF"{return Err(Error::Invalid("input is not classic AIFF"));}
    let end=start.checked_add(8+u64::from(be32(&h[4..8]))).ok_or(Error::Overflow)?;if end>physical{return Err(Error::Invalid("AIFF FORM size extends past EOF"));}if options.strict_header_consistency&&end!=physical{return Err(Error::Invalid("AIFF FORM size does not match input length"));}
    let mut position=start+12;let mut format=None;let mut declared_frames=None;let mut segments=Vec::new();
    while position+8<=end{input.seek(SeekFrom::Start(position))?;let mut ch=[0u8;8];input.read_exact(&mut ch)?;let size=u64::from(be32(&ch[4..8]));let payload=position+8;let next=payload.checked_add(size).and_then(|value|value.checked_add(size&1)).ok_or(Error::Overflow)?;if next>end{return Err(Error::Invalid("AIFF chunk extends past FORM boundary"));}
        if &ch[0..4]==b"COMM"{if format.is_some()||size<18{return Err(Error::Invalid("AIFF COMM chunk is missing, duplicate or incomplete"));}let mut b=[0u8;18];input.read_exact(&mut b)?;let channels=be16(&b[0..2]);let frames=be32(&b[2..6]);let bits=be16(&b[6..8]);let rate=decode_aiff_rate(&b[8..18])?;if channels==0||channels>options.max_channels{return Err(Error::Unsupported("AIFF channel count is unsupported"));}if !matches!(bits,8|16|24|32){return Err(Error::Unsupported("AIFF PCM width must be 8, 16, 24 or 32 bits"));}let block=channels.checked_mul(bits/8).ok_or(Error::Overflow)?;format=Some(Format{channels,rate,container_bits:bits,valid_bits:bits,block_align:block});declared_frames=Some(u64::from(frames));}
        else if &ch[0..4]==b"SSND"{if size<8{return Err(Error::Invalid("AIFF SSND chunk is incomplete"));}let mut fields=[0u8;8];input.read_exact(&mut fields)?;let offset=u64::from(be32(&fields[0..4]));if offset>size-8{return Err(Error::Invalid("AIFF SSND offset exceeds its chunk"));}segments.push(Segment{offset:payload+8+offset,size:size-8-offset});}
        position=next;
    }
    if position!=end{return Err(Error::Invalid("AIFF chunks do not consume the FORM"));}let format=format.ok_or(Error::Invalid("missing AIFF COMM chunk"))?;let audio=segments.iter().try_fold(0u64,|sum,segment|sum.checked_add(segment.size).ok_or(Error::Overflow))?;if options.strict_header_consistency&&audio/u64::from(format.block_align)!=declared_frames.ok_or(Error::Invalid("missing AIFF frame count"))?{return Err(Error::Invalid("AIFF COMM frame count does not match SSND audio"));}
    finish(physical-start,format,storage(Profile::Aiff),segments,Vec::new())
}

fn parse_caf<R:Read+Seek>(input:&mut R,options:&Options)->Result<Parsed,Error>{
    let start=input.stream_position()?;let physical=input.seek(SeekFrom::End(0))?;input.seek(SeekFrom::Start(start))?;let mut h=[0u8;8];input.read_exact(&mut h)?;if &h[0..4]!=b"caff"||u16::from_be_bytes([h[4],h[5]])!=1{return Err(Error::Invalid("input is not CAF version 1"));}
    let mut position=start+8;let mut format=None;let mut data=None;let mut store=storage(Profile::Caf);
    while position+12<=physical{input.seek(SeekFrom::Start(position))?;let mut ch=[0u8;12];input.read_exact(&mut ch)?;let signed=i64::from_be_bytes(ch[4..12].try_into().expect("eight bytes"));let payload=position+12;let size=if signed<0{physical-payload}else{signed as u64};let next=payload.checked_add(size).ok_or(Error::Overflow)?;if next>physical{return Err(Error::Invalid("CAF chunk extends past EOF"));}
        if &ch[0..4]==b"desc"{if size<32{return Err(Error::Invalid("CAF desc chunk is incomplete"));}let mut b=[0u8;32];input.read_exact(&mut b)?;let rate_f=f64::from_bits(be64(&b[0..8]));if !rate_f.is_finite()||rate_f<=0.0||rate_f.fract()!=0.0||rate_f>u32::MAX as f64{return Err(Error::Unsupported("CAF sample rate is not an exact WAVE integer"));}if &b[8..12]!=b"lpcm"{return Err(Error::Unsupported("CAF codec is not linear PCM"));}let flags=be32(&b[12..16]);if flags&1!=0||flags&4==0||flags&8==0||flags&32!=0{return Err(Error::Unsupported("CAF PCM must be signed, packed, interleaved integer PCM"));}store.endian=if flags&2!=0{Endian::Big}else{Endian::Little};let bytes_packet=be32(&b[16..20]);let frames_packet=be32(&b[20..24]);let channels32=be32(&b[24..28]);let valid=be32(&b[28..32]);if frames_packet!=1{return Err(Error::Unsupported("CAF frames-per-packet must be one"));}let channels=u16::try_from(channels32).map_err(|_|Error::Unsupported("CAF channel count is too large"))?;if channels==0||channels>options.max_channels||bytes_packet%channels32!=0{return Err(Error::Invalid("CAF channel or packet size is invalid"));}let sample_bytes=bytes_packet/channels32;if !(1..=4).contains(&sample_bytes)||valid==0||valid>sample_bytes*8{return Err(Error::Unsupported("CAF PCM width is unsupported"));}format=Some(Format{channels,rate:rate_f as u32,container_bits:(sample_bytes*8)as u16,valid_bits:valid as u16,block_align:bytes_packet as u16});}
        else if &ch[0..4]==b"data"{if size<4{return Err(Error::Invalid("CAF data chunk has no edit count"));}data=Some(Segment{offset:payload+4,size:size-4});}
        position=next;
    }
    if options.strict_header_consistency&&position!=physical{return Err(Error::Invalid("CAF chunks do not consume the input"));}
    finish(physical-start,format.ok_or(Error::Invalid("missing CAF desc chunk"))?,store,vec![data.ok_or(Error::Invalid("missing CAF data chunk"))?],Vec::new())
}

fn parse_wave64<R:Read+Seek>(input:&mut R,options:&Options)->Result<Parsed,Error>{
    let start=input.stream_position()?;let physical=input.seek(SeekFrom::End(0))?;input.seek(SeekFrom::Start(start))?;let mut h=[0u8;40];input.read_exact(&mut h)?;if h[0..16]!=W64_RIFF||h[24..40]!=W64_WAVE{return Err(Error::Invalid("input is not Sony Wave64"));}let declared=le64(&h[16..24]);if declared<40||start+declared>physical{return Err(Error::Invalid("Wave64 size exceeds EOF"));}if options.strict_header_consistency&&start+declared!=physical{return Err(Error::Invalid("Wave64 size does not match input length"));}let end=start+declared;let mut position=start+40;let mut format=None;let mut segments=Vec::new();
    while position+24<=end{input.seek(SeekFrom::Start(position))?;let mut ch=[0u8;24];input.read_exact(&mut ch)?;let size=le64(&ch[16..24]);if size<24{return Err(Error::Invalid("Wave64 chunk size is smaller than its header"));}let payload=position+24;let data_size=size-24;let next=align(position+size,8)?;if next>end{return Err(Error::Invalid("Wave64 chunk exceeds container"));}if ch[0..16]==W64_FMT{if data_size>4096{return Err(Error::Unsupported("Wave64 fmt is too large"));}let mut b=vec![0u8;data_size as usize];input.read_exact(&mut b)?;format=Some(parse_wave_format(&b,options)?);}else if ch[0..16]==W64_DATA{segments.push(Segment{offset:payload,size:data_size});}position=next;}
    finish(physical-start,format.ok_or(Error::Invalid("missing Wave64 fmt chunk"))?,storage(Profile::Wave64),segments,Vec::new())
}

fn finish(input_bytes:u64,format:Format,store:Storage,segments:Vec<Segment>,warnings:Vec<String>)->Result<Parsed,Error>{
    if segments.is_empty(){return Err(Error::Invalid("missing PCM data"));}let mut audio=0u64;for segment in &segments{if segment.size%u64::from(format.block_align)!=0{return Err(Error::Invalid("PCM data is not frame aligned"));}audio=audio.checked_add(segment.size).ok_or(Error::Overflow)?;}Ok(Parsed{input_bytes,format,storage:store,segments,audio_bytes:audio,warnings})
}

fn parse<R:Read+Seek>(input:&mut R,options:&Options)->Result<Parsed,Error>{match SOURCE{Profile::Wav|Profile::Rf64|Profile::Bw64|Profile::Bwf=>parse_riff(input,SOURCE,options),Profile::Aiff=>parse_aiff(input,options),Profile::Caf=>parse_caf(input,options),Profile::Au=>parse_au(input,options),Profile::Wave64=>parse_wave64(input,options)}}

fn wave_fmt(format:Format)->Vec<u8>{let extensible=format.valid_bits!=format.container_bits;let mut b=Vec::with_capacity(if extensible{40}else{16});b.extend_from_slice(&(if extensible{0xfffeu16}else{1u16}).to_le_bytes());b.extend_from_slice(&format.channels.to_le_bytes());b.extend_from_slice(&format.rate.to_le_bytes());b.extend_from_slice(&(format.rate*u32::from(format.block_align)).to_le_bytes());b.extend_from_slice(&format.block_align.to_le_bytes());b.extend_from_slice(&format.container_bits.to_le_bytes());if extensible{b.extend_from_slice(&22u16.to_le_bytes());b.extend_from_slice(&format.valid_bits.to_le_bytes());b.extend_from_slice(&0u32.to_le_bytes());b.extend_from_slice(&PCM_GUID);}b}

fn aiff_rate(rate:u32)->[u8;10]{let power=31-rate.leading_zeros();let exponent=(16_383+power)as u16;let mantissa=(rate as u64)<<(63-power);let mut out=[0u8;10];out[0..2].copy_from_slice(&exponent.to_be_bytes());out[2..10].copy_from_slice(&mantissa.to_be_bytes());out}
fn aiff_plan(format:Format,audio:u64)->Result<Plan,Error>{if format.valid_bits!=format.container_bits{return Err(Error::Unsupported("AIFF target requires equal valid and container widths in version 0.1"));}let frames=audio/u64::from(format.block_align);let frames=u32::try_from(frames).map_err(|_|Error::SizeLimit)?;let ssnd=audio.checked_add(8).ok_or(Error::Overflow)?;let pad=audio&1;let total=12u64.checked_add(8+18).and_then(|value|value.checked_add(8+ssnd+pad)).ok_or(Error::Overflow)?;let form=u32::try_from(total-8).map_err(|_|Error::SizeLimit)?;let ssnd=u32::try_from(ssnd).map_err(|_|Error::SizeLimit)?;let mut b=b"FORM".to_vec();b.extend_from_slice(&form.to_be_bytes());b.extend_from_slice(b"AIFFCOMM");b.extend_from_slice(&18u32.to_be_bytes());b.extend_from_slice(&format.channels.to_be_bytes());b.extend_from_slice(&frames.to_be_bytes());b.extend_from_slice(&format.valid_bits.to_be_bytes());b.extend_from_slice(&aiff_rate(format.rate));b.extend_from_slice(b"SSND");b.extend_from_slice(&ssnd.to_be_bytes());b.extend_from_slice(&0u32.to_be_bytes());b.extend_from_slice(&0u32.to_be_bytes());Ok(Plan{prefix:b,padding:pad as usize})}

fn riff_plan(profile:Profile,format:Format,audio:u64)->Result<Plan,Error>{
    let fmt=wave_fmt(format);let pad=(audio&1)as usize;let bext=profile==Profile::Bwf;let extended=matches!(profile,Profile::Rf64|Profile::Bw64);let extra=if bext{8+602}else if extended{8+28}else{0};let total=12u64.checked_add(8+fmt.len()as u64).and_then(|v|v.checked_add(extra)).and_then(|v|v.checked_add(8+audio+pad as u64)).ok_or(Error::Overflow)?;let mut b=Vec::new();
    if extended{b.extend_from_slice(if profile==Profile::Rf64{b"RF64"}else{b"BW64"});b.extend_from_slice(&u32::MAX.to_le_bytes());b.extend_from_slice(b"WAVEds64");b.extend_from_slice(&28u32.to_le_bytes());b.extend_from_slice(&(total-8).to_le_bytes());b.extend_from_slice(&audio.to_le_bytes());b.extend_from_slice(&(audio/u64::from(format.block_align)).to_le_bytes());b.extend_from_slice(&0u32.to_le_bytes());}
    else{let riff=u32::try_from(total-8).map_err(|_|Error::SizeLimit)?;b.extend_from_slice(b"RIFF");b.extend_from_slice(&riff.to_le_bytes());b.extend_from_slice(b"WAVE");}
    b.extend_from_slice(b"fmt ");b.extend_from_slice(&(fmt.len()as u32).to_le_bytes());b.extend_from_slice(&fmt);
    if bext{let mut payload=vec![0u8;602];payload[346..348].copy_from_slice(&1u16.to_le_bytes());b.extend_from_slice(b"bext");b.extend_from_slice(&602u32.to_le_bytes());b.extend_from_slice(&payload);}
    b.extend_from_slice(b"data");b.extend_from_slice(&(if extended{u32::MAX}else{u32::try_from(audio).map_err(|_|Error::SizeLimit)?}).to_le_bytes());Ok(Plan{prefix:b,padding:pad})
}

fn caf_plan(format:Format,audio:u64)->Result<Plan,Error>{let mut b=b"caff\0\x01\0\0desc".to_vec();b.extend_from_slice(&32i64.to_be_bytes());b.extend_from_slice(&(format.rate as f64).to_bits().to_be_bytes());b.extend_from_slice(b"lpcm");let flags=4u32|8u32|if format.valid_bits!=format.container_bits{16}else{0};b.extend_from_slice(&flags.to_be_bytes());b.extend_from_slice(&u32::from(format.block_align).to_be_bytes());b.extend_from_slice(&1u32.to_be_bytes());b.extend_from_slice(&u32::from(format.channels).to_be_bytes());b.extend_from_slice(&u32::from(format.valid_bits).to_be_bytes());b.extend_from_slice(b"data");let size=i64::try_from(audio.checked_add(4).ok_or(Error::Overflow)?).map_err(|_|Error::SizeLimit)?;b.extend_from_slice(&size.to_be_bytes());b.extend_from_slice(&0u32.to_be_bytes());Ok(Plan{prefix:b,padding:0})}
fn au_plan(format:Format,audio:u64)->Result<Plan,Error>{let mut b=b".snd".to_vec();b.extend_from_slice(&24u32.to_be_bytes());b.extend_from_slice(&u32::try_from(audio).map_err(|_|Error::SizeLimit)?.to_be_bytes());let encoding=match format.container_bits{8=>2u32,16=>3,24=>4,32=>5,_=>return Err(Error::Unsupported("AU target width is unsupported"))};if format.valid_bits!=format.container_bits{return Err(Error::Unsupported("AU cannot retain narrower valid-bit declarations"));}b.extend_from_slice(&encoding.to_be_bytes());b.extend_from_slice(&format.rate.to_be_bytes());b.extend_from_slice(&u32::from(format.channels).to_be_bytes());Ok(Plan{prefix:b,padding:0})}
fn wave64_plan(format:Format,audio:u64)->Result<Plan,Error>{let fmt=wave_fmt(format);let fmt_total=24+fmt.len()as u64;let data_size=24u64.checked_add(audio).ok_or(Error::Overflow)?;let data_total=align(data_size,8)?;let total=40u64.checked_add(align(fmt_total,8)?).and_then(|v|v.checked_add(data_total)).ok_or(Error::Overflow)?;let mut b=Vec::new();b.extend_from_slice(&W64_RIFF);b.extend_from_slice(&total.to_le_bytes());b.extend_from_slice(&W64_WAVE);b.extend_from_slice(&W64_FMT);b.extend_from_slice(&fmt_total.to_le_bytes());b.extend_from_slice(&fmt);while b.len()%8!=0{b.push(0);}b.extend_from_slice(&W64_DATA);b.extend_from_slice(&data_size.to_le_bytes());Ok(Plan{prefix:b,padding:(data_total-data_size)as usize})}
fn plan(format:Format,audio:u64)->Result<Plan,Error>{match TARGET{Profile::Wav|Profile::Rf64|Profile::Bw64|Profile::Bwf=>riff_plan(TARGET,format,audio),Profile::Aiff=>aiff_plan(format,audio),Profile::Caf=>caf_plan(format,audio),Profile::Au=>au_plan(format,audio),Profile::Wave64=>wave64_plan(format,audio)}}

fn transform(bytes:&mut[u8],width:usize,source:Storage,target:Storage){for sample in bytes.chunks_exact_mut(width){if width>1&&source.endian!=target.endian{sample.reverse();}let source_signed=if width==1{source.eight_bit_signed}else{true};let target_signed=if width==1{target.eight_bit_signed}else{true};if source_signed!=target_signed{let index=if width==1||target.endian==Endian::Big{0}else{width-1};sample[index]^=0x80;}}}

/// Convert the Capsule's declared source PCM container profile to its target.
pub fn convert<R:Read+Seek,W:Write>(input:&mut R,output:&mut W,options:&Options)->Result<Report,Error>{
    if !(256..=16*1024*1024).contains(&options.buffer_size){return Err(Error::Invalid("buffer_size must be between 256 bytes and 16 MiB"));}let parsed=parse(input,options)?;let plan=plan(parsed.format,parsed.audio_bytes)?;output.write_all(&plan.prefix)?;let width=usize::from(parsed.format.container_bits/8);let mut length=options.buffer_size-(options.buffer_size%usize::from(parsed.format.block_align));if length==0{length=usize::from(parsed.format.block_align);}let mut buffer=vec![0u8;length];let target_storage=storage(TARGET);
    for segment in &parsed.segments{input.seek(SeekFrom::Start(segment.offset))?;let mut remaining=segment.size;while remaining!=0{let take=remaining.min(buffer.len()as u64)as usize;input.read_exact(&mut buffer[..take])?;transform(&mut buffer[..take],width,parsed.storage,target_storage);output.write_all(&buffer[..take])?;remaining-=take as u64;}}
    if plan.padding!=0{output.write_all(&vec![0u8;plan.padding])?;}let output_bytes=plan.prefix.len()as u64+parsed.audio_bytes+plan.padding as u64;Ok(Report{input_bytes:parsed.input_bytes,output_bytes,audio_bytes:parsed.audio_bytes,channels:parsed.format.channels,sample_rate:parsed.format.rate,container_bits_per_sample:parsed.format.container_bits,valid_bits_per_sample:parsed.format.valid_bits,sample_frames:parsed.audio_bytes/u64::from(parsed.format.block_align),peak_working_memory_bytes:buffer.len()as u64+4096,warnings:parsed.warnings})
}

/// Stable one-frame fixture used only to prove removable Adapter defaults.
#[doc(hidden)]
pub fn conformance_fixture()->Vec<u8>{let format=Format{channels:1,rate:44_100,container_bits:16,valid_bits:16,block_align:2};let plan=match SOURCE{Profile::Wav|Profile::Rf64|Profile::Bw64|Profile::Bwf=>riff_plan(SOURCE,format,2).expect("fixture RIFF"),Profile::Aiff=>aiff_plan(format,2).expect("fixture AIFF"),Profile::Caf=>caf_plan(format,2).expect("fixture CAF"),Profile::Au=>au_plan(format,2).expect("fixture AU"),Profile::Wave64=>wave64_plan(format,2).expect("fixture Wave64")};let mut audio=vec![0x34,0x12];transform(&mut audio,2,Storage{endian:Endian::Little,eight_bit_signed:true},storage(SOURCE));let mut out=plan.prefix;out.extend_from_slice(&audio);out.extend(std::iter::repeat_n(0,plan.padding));out}

#[cfg(test)]
mod tests{
    use super::*;use std::io::Cursor;
    fn fixture(profile:Profile,bits:u16,canonical:&[u8])->Vec<u8>{let format=Format{channels:1,rate:44_100,container_bits:bits,valid_bits:bits,block_align:bits/8};let plan=match profile{Profile::Wav|Profile::Rf64|Profile::Bw64|Profile::Bwf=>riff_plan(profile,format,canonical.len()as u64).unwrap(),Profile::Aiff=>aiff_plan(format,canonical.len()as u64).unwrap(),Profile::Caf=>caf_plan(format,canonical.len()as u64).unwrap(),Profile::Au=>au_plan(format,canonical.len()as u64).unwrap(),Profile::Wave64=>wave64_plan(format,canonical.len()as u64).unwrap()};let mut audio=canonical.to_vec();transform(&mut audio,usize::from(bits/8),Storage{endian:Endian::Little,eight_bit_signed:true},storage(profile));let mut out=plan.prefix;out.extend_from_slice(&audio);out.extend(std::iter::repeat_n(0,plan.padding));out}
    fn decoded_target(bytes:Vec<u8>)->Vec<u8>{let mut input=Cursor::new(bytes);let parsed=match TARGET{Profile::Wav|Profile::Rf64|Profile::Bw64|Profile::Bwf=>parse_riff(&mut input,TARGET,&Options::default()).unwrap(),Profile::Aiff=>parse_aiff(&mut input,&Options::default()).unwrap(),Profile::Caf=>parse_caf(&mut input,&Options::default()).unwrap(),Profile::Au=>parse_au(&mut input,&Options::default()).unwrap(),Profile::Wave64=>parse_wave64(&mut input,&Options::default()).unwrap()};let mut audio=Vec::new();for segment in &parsed.segments{input.seek(SeekFrom::Start(segment.offset)).unwrap();let mut part=vec![0u8;segment.size as usize];input.read_exact(&mut part).unwrap();audio.extend_from_slice(&part);}transform(&mut audio,usize::from(parsed.format.container_bits/8),parsed.storage,Storage{endian:Endian::Little,eight_bit_signed:true});audio}
    #[test]fn preserves_sixteen_bit_pcm(){let source=fixture(SOURCE,16,&[0x34,0x12,0xcc,0xed]);let mut output=Vec::new();let report=convert(&mut Cursor::new(source),&mut output,&Options::default()).unwrap();assert_eq!(decoded_target(output),[0x34,0x12,0xcc,0xed]);assert_eq!(report.sample_frames,2);}
    #[test]fn preserves_eight_bit_levels_across_signedness_conventions(){let source=fixture(SOURCE,8,&[0x80,0x00,0x7f]);let mut output=Vec::new();convert(&mut Cursor::new(source),&mut output,&Options::default()).unwrap();assert_eq!(decoded_target(output),[0x80,0x00,0x7f]);}
    #[test]fn rejects_wrong_signature(){let error=convert(&mut Cursor::new(vec![0u8;64]),&mut Vec::new(),&Options::default()).unwrap_err();assert!(matches!(error,Error::Invalid(_)|Error::Io(_)));}
    #[test]fn strict_defaults_reject_trailing_bytes(){let mut source=fixture(SOURCE,16,&[0,0]);source.push(0);let result=convert(&mut Cursor::new(source),&mut Vec::new(),&Options::default());assert!(result.is_err());}
}
