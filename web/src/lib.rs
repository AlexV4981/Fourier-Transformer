use fourier_transformer::{Drawing,gray_drawing,decoded_drawing_mode,manual,export_html,MAX_PIXELS};
use wasm_bindgen::prelude::*;
use std::cell::RefCell;
use std::io::Cursor;
thread_local!{static DRAWING:RefCell<Option<Drawing>>=const{RefCell::new(None)};}
fn err(s:impl ToString)->JsValue{JsValue::from_str(&s.to_string())}
fn store(d:Drawing)->Result<String,JsValue>{
    let paths:Vec<_>=d.paths.iter().map(|p|serde_json::json!({
        "name":p.name,"color":format!("#{:02x}{:02x}{:02x}",p.color[0],p.color[1],p.color[2]),"preview":p.preview.iter().map(|v|[v.re,v.im]).collect::<Vec<_>>(),
        "coeff":p.coeff.iter().map(|c|serde_json::json!([c.freq,c.amp,c.phase])).collect::<Vec<_>>()
    })).collect();
    let result=serde_json::json!({"paths":paths,"limited":d.limited,"candidates":d.candidates,"pathLimit":d.path_limit,"coefficientLimit":d.coefficient_limit,"subjectDetected":d.subject_detected}).to_string();
    DRAWING.with(|v|*v.borrow_mut()=Some(d));Ok(result)
}
#[wasm_bindgen]
pub fn clear(){DRAWING.with(|v|*v.borrow_mut()=None);}
#[wasm_bindgen]
pub fn from_gray(w:u32,h:u32,bytes:&[u8],detail:usize,cap:usize,solid:bool)->Result<String,JsValue>{
    clear();
    if u64::from(w)*u64::from(h)>MAX_PIXELS || w==0 || h==0{return Err(err("Maximum image size is 4 MP"));}
    let image=image::GrayImage::from_raw(w,h,bytes.to_vec()).ok_or_else(||err("Invalid grayscale buffer"))?;
    store(gray_drawing(image,detail,cap,solid).map_err(err)?)
}
#[wasm_bindgen]
pub fn from_image(bytes:&[u8],detail:usize,cap:usize,mode:u8,sensitivity:u8)->Result<String,JsValue>{
    clear();
    if bytes.len()>16*1024*1024{return Err(err("Maximum file size is 16 MiB"));}
    let open=||image::ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(err);
    let (w,h)=open()?.into_dimensions().map_err(err)?;
    if w==0||h==0||u64::from(w)*u64::from(h)>MAX_PIXELS{return Err(err("Image exceeds 4 MP; resize it first"));}
    let mut reader=open()?;let mut limits=image::Limits::default();
    limits.max_image_width=Some(w);limits.max_image_height=Some(h);limits.max_alloc=Some(64*1024*1024);reader.limits(limits);
    store(decoded_drawing_mode(reader.decode().map_err(err)?,detail,cap,mode,sensitivity).map_err(err)?)
}
#[wasm_bindgen]
pub fn equation(index:usize)->Result<String,JsValue>{
    DRAWING.with(|v|v.borrow().as_ref().map(|d|manual(d,index)).ok_or_else(||err("Calculate first")))
}
#[wasm_bindgen]
pub fn paste_payload(index:usize)->Result<String,JsValue>{
    DRAWING.with(|v|{
        let b=v.borrow();let d=b.as_ref().ok_or_else(||err("Calculate first"))?;
        Ok(serde_json::json!({
            "expression":fourier_transformer::paste_expression(d,index),
            "bound":if index>0&&index<=d.paths.len(){fourier_transformer::path_bound(index)}else{String::new()}
        }).to_string())
    })
}
#[wasm_bindgen]
pub fn html()->Result<String,JsValue>{
    DRAWING.with(|v|{
        let b=v.borrow();let d=b.as_ref().ok_or_else(||err("Calculate first"))?;
        let mut out=vec![];export_html(&mut out,d).map_err(err)?;
        String::from_utf8(out).map_err(err)
    })
}
