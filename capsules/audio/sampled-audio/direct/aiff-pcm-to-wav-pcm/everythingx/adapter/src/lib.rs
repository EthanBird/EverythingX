#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io::{self, Cursor, Read, Write};
use aiff_pcm_to_wav_pcm::{Error as CapsuleError, MetadataPolicy, Options};
use everythingx_protocol::{AdapterError, AdapterErrorKind, AdapterHandshake, CapabilityDescriptor, CapsuleIdentity, InvocationRequest, InvocationResult, InvocationStatus, LossLevel, Measurements, ProtocolVersion, Provenance, StaticAdapter};

pub const ADAPTER_ID: &str = "adapter:aiff-pcm-to-wav-pcm-static";
pub const CAPABILITY_ID: &str = "capability:aiff-pcm-to-wav-pcm/pcm-exact/native-portable";
pub struct AiffPcmToWavPcmAdapter;

fn defaults() -> BTreeMap<String,String> { BTreeMap::from([("buffer_size".into(),"65536".into()),("max_channels".into(),"256".into()),("max_metadata_bytes".into(),"1048576".into()),("metadata".into(),"common-text".into()),("strict_header_consistency".into(),"true".into())]) }
fn descriptor() -> CapabilityDescriptor { CapabilityDescriptor { capability_id: CAPABILITY_ID.into(), source_formats: vec!["exfmt:audio:aiff-pcm".into()], target_formats: vec!["exfmt:audio:wav-pcm".into()], strategy:"pcm-exact".into(), backend:"native-portable".into(), default_options:defaults(), defaults_are_runnable:true, streaming:false, seek_required:false } }
fn bad(message:impl Into<String>)->AdapterError{AdapterError::new(AdapterErrorKind::InvalidOptions,message)}
fn parse(request:&InvocationRequest)->Result<Options,AdapterError>{
    if request.capability_id!=CAPABILITY_ID{return Err(AdapterError::new(AdapterErrorKind::UnsupportedCapability,"unsupported capability"));}
    let mut o=Options::default(); for(name,value)in &request.options{match name.as_str(){
        "metadata"=>o.metadata=match value.as_str(){"common-text"=>MetadataPolicy::CommonText,"discard"=>MetadataPolicy::Discard,_=>return Err(bad("metadata must be common-text or discard"))},
        "strict_header_consistency"=>o.strict_header_consistency=match value.as_str(){"true"=>true,"false"=>false,_=>return Err(bad("strict_header_consistency must be true or false"))},
        "buffer_size"=>o.buffer_size=value.parse().map_err(|_|bad("buffer_size must be usize"))?,"max_metadata_bytes"=>o.max_metadata_bytes=value.parse().map_err(|_|bad("max_metadata_bytes must be u64"))?,"max_channels"=>o.max_channels=value.parse().map_err(|_|bad("max_channels must be u16"))?,_=>return Err(bad(format!("unknown option {name}"))),
    }}Ok(o)
}
fn option_map(o:&Options)->BTreeMap<String,String>{BTreeMap::from([("buffer_size".into(),o.buffer_size.to_string()),("max_channels".into(),o.max_channels.to_string()),("max_metadata_bytes".into(),o.max_metadata_bytes.to_string()),("metadata".into(),match o.metadata{MetadataPolicy::CommonText=>"common-text",MetadataPolicy::Discard=>"discard"}.into()),("strict_header_consistency".into(),o.strict_header_consistency.to_string())])}

struct LimitedWriter<'a>{inner:&'a mut dyn Write,remaining:u64,exceeded:bool}
impl Write for LimitedWriter<'_>{fn write(&mut self,bytes:&[u8])->io::Result<usize>{if bytes.len()as u64>self.remaining{self.exceeded=true;return Err(io::Error::other("output budget exceeded"));}let written=self.inner.write(bytes)?;self.remaining-=written as u64;Ok(written)}fn flush(&mut self)->io::Result<()>{self.inner.flush()}}

impl StaticAdapter for AiffPcmToWavPcmAdapter{
    fn handshake(&self)->AdapterHandshake{AdapterHandshake{protocol:ProtocolVersion::CURRENT,adapter_id:ADAPTER_ID.into(),adapter_version:"0.1.0".into(),capsule:CapsuleIdentity{id:"capsule:aiff-pcm-to-wav-pcm".into(),version:"0.1.0".into(),content_hash:None},capabilities:vec![descriptor()]}}
    fn invoke(&self,request:&InvocationRequest,input:&mut dyn Read,output:&mut dyn Write)->Result<InvocationResult,AdapterError>{
        let options=parse(request)?;let mut bytes=Vec::new();input.take(request.resource_budget.max_memory_bytes.saturating_add(1)).read_to_end(&mut bytes).map_err(|e|AdapterError::new(AdapterErrorKind::Io,e.to_string()))?;if bytes.len()as u64>request.resource_budget.max_memory_bytes{return Err(AdapterError::new(AdapterErrorKind::ResourceLimit,"input exceeds memory budget"));}let source_len=bytes.len()as u64;let mut source=Cursor::new(bytes);let mut limited=LimitedWriter{inner:output,remaining:request.resource_budget.max_output_bytes,exceeded:false};let report=aiff_pcm_to_wav_pcm::convert(&mut source,&mut limited,&options).map_err(|e|match e{CapsuleError::Io(io)if limited.exceeded=>AdapterError::new(AdapterErrorKind::ResourceLimit,io.to_string()),CapsuleError::Io(io)=>AdapterError::new(AdapterErrorKind::Io,io.to_string()),other=>AdapterError::new(AdapterErrorKind::InvalidInput,other.to_string())})?;let handshake=self.handshake();
        Ok(InvocationResult{status:InvocationStatus::Succeeded,effects:BTreeMap::from([("format".into(),"wav-pcm".into()),("byte_order".into(),"little-endian".into())]),losses:BTreeMap::from([("payload".into(),LossLevel::None),("temporal".into(),LossLevel::None),("structure".into(),LossLevel::Normalized),("metadata".into(),LossLevel::Bounded)]),measurements:Measurements{input_bytes:Some(report.input_bytes),output_bytes:Some(report.output_bytes),peak_memory_bytes:Some(source_len+report.peak_working_memory_bytes),..Measurements::default()},capsule_report:BTreeMap::from([("channels".into(),report.channels.to_string()),("sample_rate".into(),report.sample_rate.to_string()),("container_bits_per_sample".into(),report.container_bits_per_sample.to_string()),("valid_bits_per_sample".into(),report.valid_bits_per_sample.to_string()),("sample_frames".into(),report.sample_frames.to_string()),("source_sound_chunks".into(),report.source_sound_chunks.to_string()),("metadata_chunks_preserved".into(),report.metadata_chunks_preserved.to_string())]),warnings:report.warnings,provenance:Provenance{capsule:handshake.capsule,adapter_id:handshake.adapter_id,adapter_version:handshake.adapter_version,capability_id:CAPABILITY_ID.into(),strategy:"pcm-exact".into(),backend:"native-portable".into(),effective_options:option_map(&options)}})
    }
}

#[cfg(test)]mod tests{use super::*;use everythingx_kernel::Kernel;fn one_sample()->Vec<u8>{let mut body=b"AIFFCOMM".to_vec();body.extend_from_slice(&18_u32.to_be_bytes());body.extend_from_slice(&1_u16.to_be_bytes());body.extend_from_slice(&1_u32.to_be_bytes());body.extend_from_slice(&8_u16.to_be_bytes());body.extend_from_slice(&[0x40,0x0e,0xac,0x44,0,0,0,0,0,0]);body.extend_from_slice(b"SSND");body.extend_from_slice(&9_u32.to_be_bytes());body.extend_from_slice(&[0;8]);body.push(0);body.push(0);let mut out=b"FORM".to_vec();out.extend_from_slice(&(body.len()as u32).to_be_bytes());out.extend_from_slice(&body);out}#[test]fn kernel_defaults_convert(){let mut kernel=Kernel::default();kernel.register(Box::new(AiffPcmToWavPcmAdapter)).unwrap();let source=one_sample();let mut input=&source[..];let mut output=Vec::new();let result=kernel.invoke_defaults(ADAPTER_ID,CAPABILITY_ID,&mut input,&mut output).unwrap();assert_eq!(result.status,InvocationStatus::Succeeded);assert_eq!(&output[..4],b"RIFF");}}
