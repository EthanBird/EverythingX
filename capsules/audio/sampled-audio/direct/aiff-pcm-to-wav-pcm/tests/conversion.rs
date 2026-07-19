use std::io::Cursor;
use aiff_pcm_to_wav_pcm::{convert, Error, MetadataPolicy, Options};

fn extended(rate: u32) -> [u8; 10] { let power = 31 - rate.leading_zeros(); let exponent = (16_383 + power) as u16; let mantissa = (rate as u64) << (63 - power); let mut b = [0; 10]; b[..2].copy_from_slice(&exponent.to_be_bytes()); b[2..].copy_from_slice(&mantissa.to_be_bytes()); b }
fn aiff(channels: u16, frames: u32, bits: u16, rate: u32, sound: &[u8], metadata: &[([u8;4], &[u8])]) -> Vec<u8> {
    let mut body = b"AIFFCOMM".to_vec(); body.extend_from_slice(&18_u32.to_be_bytes()); body.extend_from_slice(&channels.to_be_bytes()); body.extend_from_slice(&frames.to_be_bytes()); body.extend_from_slice(&bits.to_be_bytes()); body.extend_from_slice(&extended(rate));
    for (id, bytes) in metadata { body.extend_from_slice(id); body.extend_from_slice(&(bytes.len() as u32).to_be_bytes()); body.extend_from_slice(bytes); if bytes.len() & 1 != 0 { body.push(0); } }
    body.extend_from_slice(b"SSND"); body.extend_from_slice(&((sound.len() + 8) as u32).to_be_bytes()); body.extend_from_slice(&[0;8]); body.extend_from_slice(sound); if sound.len() & 1 != 0 { body.push(0); }
    let mut out = b"FORM".to_vec(); out.extend_from_slice(&(body.len() as u32).to_be_bytes()); out.extend_from_slice(&body); out
}
fn data(wave: &[u8]) -> &[u8] { let pos = wave.windows(4).position(|w| w == b"data").unwrap(); let size = u32::from_le_bytes(wave[pos+4..pos+8].try_into().unwrap()) as usize; &wave[pos+8..pos+8+size] }

#[test] fn signed_eight_bit_becomes_wave_unsigned() { let mut out=Vec::new(); let report=convert(&mut Cursor::new(aiff(1,3,8,44_100,&[0x80,0,0x7f],&[])),&mut out,&Options::default()).unwrap(); assert_eq!(data(&out),[0,0x80,0xff]); assert_eq!(report.sample_frames,3); }
#[test] fn reverses_sixteen_and_twenty_four_bit_samples() { for (bits,src,want) in [(16,vec![0x12,0x34],vec![0x34,0x12]),(24,vec![1,2,3],vec![3,2,1])] { let mut out=Vec::new(); convert(&mut Cursor::new(aiff(1,1,bits,48_000,&src,&[])),&mut out,&Options::default()).unwrap(); assert_eq!(data(&out),want); } }
#[test] fn maps_text_metadata_to_info() { let mut out=Vec::new(); let report=convert(&mut Cursor::new(aiff(1,1,8,8_000,&[0],&[(*b"NAME",b"Song"),(*b"AUTH",b"Artist")])),&mut out,&Options::default()).unwrap(); assert!(out.windows(4).any(|w|w==b"INFO")); assert!(out.windows(4).any(|w|w==b"INAM")); assert_eq!(report.metadata_chunks_preserved,2); }
#[test] fn discard_metadata_is_runnable() { let mut out=Vec::new(); let options=Options{metadata:MetadataPolicy::Discard,..Options::default()}; let report=convert(&mut Cursor::new(aiff(1,1,8,8_000,&[0],&[(*b"NAME",b"Song")])),&mut out,&options).unwrap(); assert!(!out.windows(4).any(|w|w==b"LIST")); assert_eq!(report.metadata_chunks_found,1); }
#[test] fn strict_mode_rejects_frame_mismatch() { let error=convert(&mut Cursor::new(aiff(1,2,16,8_000,&[0,0],&[])),&mut Vec::new(),&Options::default()).unwrap_err(); assert!(matches!(error,Error::InvalidAiff(_))); }
