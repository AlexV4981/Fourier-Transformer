use fourier_transformer::{color_drawing,decoded_drawing,gray_drawing,export_html,manual,MAX_COEFFICIENTS,MAX_PATHS};
use image::{DynamicImage,Rgb,RgbImage,Rgba,RgbaImage,ImageFormat};
use std::io::Cursor;
fn check(d:&fourier_transformer::Drawing,cap:usize,detail:usize){
    assert!(!d.paths.is_empty());
    assert!(d.paths.len()<=cap.min(MAX_PATHS));
    assert!(d.paths.iter().map(|p|p.coeff.len()).sum::<usize>()<=MAX_COEFFICIENTS);
    for p in &d.paths {
        assert!(!p.preview.is_empty());assert!(p.coeff.len()<=detail);
        assert!(p.preview.iter().all(|v|v.re.is_finite()&&v.im.is_finite()));
        assert!(p.coeff.iter().all(|c|c.amp.is_finite()&&c.phase.is_finite()&&c.amp>=0.));
    }
}
fn art(w:u32,h:u32)->RgbImage{
    RgbImage::from_fn(w,h,|x,y|{
        let cx=x as i64-w as i64/2;let cy=y as i64-h as i64/2;
        if cx*cx+cy*cy<(h as i64/3).pow(2){Rgb([225,30,45])}
        else if x<w/3{Rgb([20,120,235])}
        else if y<h/3{Rgb([240,210,35])}
        else{Rgb([245,245,245])}
    })
}
#[test]fn all_formats_and_settings(){
    let original=DynamicImage::ImageRgb8(art(320,240));
    for format in [ImageFormat::Png,ImageFormat::Jpeg,ImageFormat::Bmp,ImageFormat::WebP]{
        let mut bytes=Cursor::new(Vec::new());original.write_to(&mut bytes,format).unwrap();
        let decoded=image::load_from_memory(bytes.get_ref()).unwrap();
        for (detail,cap,sensitivity) in [(50,5,35),(150,40,65),(500,200,90)]{
            let d=decoded_drawing(decoded.clone(),detail,cap,true,sensitivity).unwrap();
            check(&d,cap,detail);
        }
    }
}
#[test]fn repeated_noisy_high_detail(){
    let rgb=RgbImage::from_fn(800,600,|x,y|{
        let seed=x.wrapping_mul(374761393).wrapping_add(y.wrapping_mul(668265263));
        let hash=(seed^(seed>>13)).wrapping_mul(1274126177);
        let v=(hash^(hash>>16)) as u8;
        if (x/19+y/23)%2==0{Rgb([v,30,200])}else{Rgb([220,v,40])}
    });
    for _ in 0..3{let d=color_drawing(&rgb,500,200,90).unwrap();check(&d,200,500);}
}
#[test]fn large_image_and_hard_limits(){
    let d=decoded_drawing(DynamicImage::ImageRgb8(art(2000,2000)),500,200,true,65).unwrap();
    check(&d,200,500);
    assert!(decoded_drawing(DynamicImage::new_rgb8(2001,2000),100,80,true,65).is_err());
    assert!(decoded_drawing(DynamicImage::new_rgb8(0,10),100,80,true,65).is_err());
    assert!(color_drawing(&RgbImage::new(2,2),100,80,65).is_err());
}
#[test]fn transparent_hidden_rgb_and_blank(){
    let invisible=RgbaImage::from_fn(200,200,|x,y|Rgba([(x%255) as u8,(y%255) as u8,255,0]));
    assert!(decoded_drawing(DynamicImage::ImageRgba8(invisible),100,80,true,65).is_err());
    let visible=RgbaImage::from_fn(200,200,|x,y|if (30..170).contains(&x)&&(30..170).contains(&y){Rgba([220,10,60,255])}else{Rgba([0,255,0,0])});
    let d=decoded_drawing(DynamicImage::ImageRgba8(visible),100,80,true,65).unwrap();check(&d,80,100);
    assert!(color_drawing(&RgbImage::from_pixel(100,100,Rgb([128,128,128])),100,80,100).is_err());
}
#[test]fn classic_and_colored_export(){
    let rgb=art(480,320);
    let d=color_drawing(&rgb,200,80,65).unwrap();check(&d,80,200);
    let classic=gray_drawing(DynamicImage::ImageRgb8(rgb).into_luma8(),150,80,false).unwrap();check(&classic,80,150);
    let mut out=vec![];export_html(&mut out,&d).unwrap();
    let html=String::from_utf8(out).unwrap();
    for (i,p) in d.paths.iter().enumerate(){
        assert!(html.contains(&format!("path_{i}")));
        assert!(html.contains(&format!("#{:02x}{:02x}{:02x}",p.color[0],p.color[1],p.color[2])));
        assert!(manual(&d,i+1).contains("Bounds:"));
    }
    assert!(manual(&d,d.paths.len()+1).contains("\\le"));
}
