#![forbid(unsafe_code)]

mod png_native;

use std::fmt;
use std::io::{self, Read, Write};

const OPERATION: Operation = Operation::Validate;
const DEFAULT_MAX_PIXELS: u64 = 100_000_000;
const DEFAULT_MAX_INPUT_BYTES: u64 = 512 * 1024 * 1024;
const DEFAULT_MAX_INFLATE_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum Operation { Validate, Normalize, Crop, Pad, FlipHorizontal, FlipVertical, Rotate90, Rotate180, Rotate270, AlphaPremultiply, AlphaUnpremultiply }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterStrategy { None, Sub, Up, Average, Paeth, Adaptive }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Background { pub red: u16, pub green: u16, pub blue: u16, pub alpha: u16 }

impl Default for Background { fn default() -> Self { Self { red: 0, green: 0, blue: 0, alpha: 0 } } }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub max_pixels: u64,
    pub max_input_bytes: u64,
    pub max_inflate_bytes: u64,
    pub strict_crc: bool,
    pub strict_trailing_data: bool,
    pub filter: FilterStrategy,
    /// A zero width/height means the remaining source extent.
    pub crop_x: u32,
    pub crop_y: u32,
    pub crop_width: u32,
    pub crop_height: u32,
    pub pad_left: u32,
    pub pad_right: u32,
    pub pad_top: u32,
    pub pad_bottom: u32,
    pub background: Background,
}

impl Default for Options {
    fn default() -> Self {
        Self { max_pixels: DEFAULT_MAX_PIXELS, max_input_bytes: DEFAULT_MAX_INPUT_BYTES, max_inflate_bytes: DEFAULT_MAX_INFLATE_BYTES,
            strict_crc: true, strict_trailing_data: true, filter: FilterStrategy::Adaptive,
            crop_x: 0, crop_y: 0, crop_width: 0, crop_height: 0,
            pad_left: 0, pad_right: 0, pad_top: 0, pad_bottom: 0, background: Background::default() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub input_bytes: u64, pub output_bytes: u64, pub source_width: u32, pub source_height: u32,
    pub width: u32, pub height: u32, pub pixels: u64, pub source_color_type: u8,
    pub source_bit_depth: u8, pub source_interlaced: bool, pub target_bit_depth: u8,
    pub operation: &'static str, pub strategy: &'static str, pub backend: &'static str,
    pub peak_working_memory_bytes: u64, pub warnings: Vec<String>,
}

#[derive(Debug)]
pub enum Error {
    InvalidOptions(&'static str), InputTooLarge { bytes: u64, limit: u64 },
    PixelLimitExceeded { pixels: u64, limit: u64 }, InvalidInput(String),
    IntegerOverflow(&'static str), Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidOptions(v)=>write!(f,"invalid options: {v}"),
            Self::InputTooLarge{bytes,limit}=>write!(f,"input has {bytes} bytes, exceeding {limit}"),
            Self::PixelLimitExceeded{pixels,limit}=>write!(f,"image has {pixels} pixels, exceeding {limit}"),
            Self::InvalidInput(v)=>write!(f,"invalid PNG input: {v}"),
            Self::IntegerOverflow(v)=>write!(f,"integer overflow while computing {v}"),
            Self::Io(v)=>fmt::Display::fmt(v,f),
        }
    }
}
impl std::error::Error for Error { fn source(&self)->Option<&(dyn std::error::Error+'static)>{match self{Self::Io(v)=>Some(v),_=>None}} }
impl From<io::Error> for Error { fn from(value:io::Error)->Self{Self::Io(value)} }

fn filter(value: FilterStrategy) -> png_native::Filter { match value { FilterStrategy::None=>png_native::Filter::None,FilterStrategy::Sub=>png_native::Filter::Sub,FilterStrategy::Up=>png_native::Filter::Up,FilterStrategy::Average=>png_native::Filter::Average,FilterStrategy::Paeth=>png_native::Filter::Paeth,FilterStrategy::Adaptive=>png_native::Filter::Adaptive } }

fn operation_name() -> &'static str { match OPERATION { Operation::Validate=>"validate",Operation::Normalize=>"normalize",Operation::Crop=>"crop",Operation::Pad=>"pad",Operation::FlipHorizontal=>"flip-horizontal",Operation::FlipVertical=>"flip-vertical",Operation::Rotate90=>"rotate-90-clockwise",Operation::Rotate180=>"rotate-180",Operation::Rotate270=>"rotate-270-clockwise",Operation::AlphaPremultiply=>"alpha-premultiply",Operation::AlphaUnpremultiply=>"alpha-unpremultiply" } }

fn checked_target(width:u32,height:u32,options:&Options)->Result<usize,Error>{if width==0||height==0{return Err(Error::InvalidOptions("target dimensions must be non-zero"));}let pixels=width as u64*height as u64;if pixels>options.max_pixels{return Err(Error::PixelLimitExceeded{pixels,limit:options.max_pixels});}usize::try_from(pixels).map_err(|_|Error::IntegerOverflow("target pixel allocation"))}

fn transform(mut image:png_native::Image,options:&Options)->Result<png_native::Image,Error>{
    use png_native::Pixel16;
    let w=image.width as usize;let h=image.height as usize;
    match OPERATION {
        Operation::Validate|Operation::Normalize=>{}
        Operation::Crop=>{let x=options.crop_x;let y=options.crop_y;if x>=image.width||y>=image.height{return Err(Error::InvalidOptions("crop origin is outside the image"));}let width=if options.crop_width==0{image.width-x}else{options.crop_width};let height=if options.crop_height==0{image.height-y}else{options.crop_height};if x.checked_add(width).is_none_or(|v|v>image.width)||y.checked_add(height).is_none_or(|v|v>image.height){return Err(Error::InvalidOptions("crop rectangle exceeds the image"));}let count=checked_target(width,height,options)?;let mut pixels=Vec::with_capacity(count);for row in y as usize..(y+height)as usize{pixels.extend_from_slice(&image.pixels[row*w+x as usize..row*w+(x+width)as usize]);}image.width=width;image.height=height;image.pixels=pixels;}
        Operation::Pad=>{let width=image.width.checked_add(options.pad_left).and_then(|v|v.checked_add(options.pad_right)).ok_or(Error::IntegerOverflow("padded width"))?;let height=image.height.checked_add(options.pad_top).and_then(|v|v.checked_add(options.pad_bottom)).ok_or(Error::IntegerOverflow("padded height"))?;let count=checked_target(width,height,options)?;let bg=Pixel16{r:options.background.red,g:options.background.green,b:options.background.blue,a:options.background.alpha};let mut pixels=vec![bg;count];for y in 0..h{let dst=(y+options.pad_top as usize)*width as usize+options.pad_left as usize;pixels[dst..dst+w].copy_from_slice(&image.pixels[y*w..(y+1)*w]);}image.width=width;image.height=height;image.pixels=pixels;}
        Operation::FlipHorizontal=>{for row in image.pixels.chunks_exact_mut(w){row.reverse();}}
        Operation::FlipVertical=>{for y in 0..h/2{for x in 0..w{image.pixels.swap(y*w+x,(h-1-y)*w+x);}}}
        Operation::Rotate90|Operation::Rotate270=>{let mut pixels=vec![Pixel16::default();checked_target(image.height,image.width,options)?];for y in 0..h{for x in 0..w{let (nx,ny)=if OPERATION==Operation::Rotate90{(h-1-y,x)}else{(y,w-1-x)};pixels[ny*h+nx]=image.pixels[y*w+x];}}std::mem::swap(&mut image.width,&mut image.height);image.pixels=pixels;}
        Operation::Rotate180=>{image.pixels.reverse();}
        Operation::AlphaPremultiply=>{for p in &mut image.pixels{let a=p.a as u32;p.r=((p.r as u32*a+32_767)/65_535)as u16;p.g=((p.g as u32*a+32_767)/65_535)as u16;p.b=((p.b as u32*a+32_767)/65_535)as u16;}}
        Operation::AlphaUnpremultiply=>{for p in &mut image.pixels{if p.a==0{p.r=0;p.g=0;p.b=0;}else{let a=p.a as u32;let expand=|v:u16|->u16{((v as u32*65_535+a/2)/a).min(65_535)as u16};p.r=expand(p.r);p.g=expand(p.g);p.b=expand(p.b);}}}
    }
    image.interlaced=false;image.source_color_type=if image.pixels.iter().any(|p|p.a!=65_535){6}else{2};image.source_channels=if image.source_color_type==6{4}else{3};Ok(image)
}

/// Validate or transform one PNG using only this crate's native implementation.
/// The complete source is validated before any target bytes are committed.
pub fn convert<R:Read+?Sized,W:Write+?Sized>(input:&mut R,output:&mut W,options:&Options)->Result<Report,Error>{
    if options.max_pixels==0||options.max_input_bytes==0||options.max_inflate_bytes==0{return Err(Error::InvalidOptions("resource limits must be non-zero"));}
    let mut source=Vec::new();input.take(options.max_input_bytes.saturating_add(1)).read_to_end(&mut source)?;if source.len()as u64>options.max_input_bytes{return Err(Error::InputTooLarge{bytes:source.len()as u64,limit:options.max_input_bytes});}
    let decoded=png_native::decode(&source,&png_native::DecodeOptions{max_pixels:options.max_pixels,max_inflate_bytes:options.max_inflate_bytes,strict_crc:options.strict_crc,strict_trailing_data:options.strict_trailing_data}).map_err(|e|match e{png_native::Error::Limit("pixel count")=>Error::PixelLimitExceeded{pixels:options.max_pixels.saturating_add(1),limit:options.max_pixels},other=>Error::InvalidInput(other.to_string())})?;
    let source_width=decoded.width;let source_height=decoded.height;let source_color_type=decoded.source_color_type;let source_bit_depth=decoded.source_bit_depth;let source_interlaced=decoded.interlaced;
    let transformed=transform(decoded,options)?;let bytes=if OPERATION==Operation::Validate{source.clone()}else{png_native::encode(&transformed,filter(options.filter)).map_err(|e|Error::InvalidInput(e.to_string()))?};output.write_all(&bytes)?;
    let pixel_memory=transformed.pixels.len()as u64*8;let peak=source.len()as u64+bytes.len()as u64+pixel_memory;
    Ok(Report{input_bytes:source.len()as u64,output_bytes:bytes.len()as u64,source_width,source_height,width:transformed.width,height:transformed.height,pixels:transformed.pixels.len()as u64,source_color_type,source_bit_depth,source_interlaced,target_bit_depth:if OPERATION==Operation::Validate{source_bit_depth}else if source_bit_depth==16{16}else{8},operation:operation_name(),strategy:"png-native-canonical",backend:"native-portable",peak_working_memory_bytes:peak,warnings:transformed.warnings})
}

#[doc(hidden)]
pub fn conformance_fixture()->Vec<u8>{let image=png_native::Image{width:3,height:2,source_channels:4,source_bit_depth:8,source_color_type:6,interlaced:false,pixels:vec![png_native::Pixel16{r:0,g:0,b:0,a:65_535},png_native::Pixel16{r:65_535,g:0,b:0,a:32_896},png_native::Pixel16{r:0,g:65_535,b:0,a:65_535},png_native::Pixel16{r:0,g:0,b:65_535,a:65_535},png_native::Pixel16{r:65_535,g:65_535,b:0,a:65_535},png_native::Pixel16{r:65_535,g:65_535,b:65_535,a:65_535}],warnings:Vec::new()};png_native::encode(&image,png_native::Filter::Adaptive).expect("fixture PNG")}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn defaults_are_runnable(){let source=conformance_fixture();let mut input=&source[..];let mut output=Vec::new();let report=convert(&mut input,&mut output,&Options::default()).unwrap();assert!(!output.is_empty());assert_eq!(report.operation,operation_name());}
    #[test]fn output_is_independently_decodable(){let source=conformance_fixture();let mut input=&source[..];let mut output=Vec::new();convert(&mut input,&mut output,&Options::default()).unwrap();let image=png_native::decode(&output,&png_native::DecodeOptions{max_pixels:100,max_inflate_bytes:10_000,strict_crc:true,strict_trailing_data:true}).unwrap();assert!(image.width>0&&image.height>0);}
    #[test]fn malformed_crc_is_rejected_before_write(){let mut source=conformance_fixture();source[29]^=1;let mut input=&source[..];let mut output=Vec::new();assert!(convert(&mut input,&mut output,&Options::default()).is_err());assert!(output.is_empty());}
    #[test]fn operation_contract(){let source=conformance_fixture();let mut options=Options::default();if OPERATION==Operation::Crop{options.crop_x=1;options.crop_width=2;options.crop_height=1;}if OPERATION==Operation::Pad{options.pad_left=1;options.pad_bottom=2;}let mut input=&source[..];let mut output=Vec::new();let report=convert(&mut input,&mut output,&options).unwrap();match OPERATION{Operation::Crop=>assert_eq!((report.width,report.height),(2,1)),Operation::Pad=>assert_eq!((report.width,report.height),(4,4)),Operation::Rotate90|Operation::Rotate270=>assert_eq!((report.width,report.height),(2,3)),_=>assert_eq!((report.width,report.height),(3,2))}}
    #[test]fn input_limit_is_enforced(){let source=conformance_fixture();let mut options=Options::default();options.max_input_bytes=8;let mut input=&source[..];let mut output=Vec::new();assert!(matches!(convert(&mut input,&mut output,&options),Err(Error::InputTooLarge{..})));}
}
