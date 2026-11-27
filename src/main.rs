use eframe::egui;
use fourier_transformer::{Drawing, text_drawing, image_drawing, manual, export_html};
use std::{path::PathBuf, sync::mpsc, io::{BufWriter,Write}};
struct App {
    image_mode:bool,text:String,file:Option<PathBuf>,detail:usize,contours:usize,color:bool,sensitivity:u8,
    drawing:Option<Drawing>,selected:usize,output:String,status:String,
    worker:Option<mpsc::Receiver<Result<Drawing,String>>>,
}
impl Default for App {
    fn default()->Self{Self{image_mode:false,text:"I [heart] MATH".into(),file:None,detail:150,contours:80,color:true,sensitivity:65,drawing:None,selected:0,output:String::new(),status:String::new(),worker:None}}
}
impl App {
    fn calculate(&mut self,ctx:egui::Context){
        let image=self.image_mode;let text=self.text.clone();let file=self.file.clone();let detail=self.detail;let contours=self.contours;let color=self.color;let sensitivity=self.sensitivity;
        self.drawing=None;self.output=String::new();self.selected=0;self.status="Calculating…".into();
        let (tx,rx)=mpsc::channel();self.worker=Some(rx);
        std::thread::spawn(move||{
            let result=if image{file.ok_or_else(||"Select an image".to_string()).and_then(|p|image_drawing(&p,detail,contours,color,sensitivity))}else{text_drawing(&text,detail)};
            let _=tx.send(result);ctx.request_repaint();
        });
    }
    fn save(&mut self,html:bool){
        let Some(d)=&self.drawing else{return;};
        let Some(path)=rfd::FileDialog::new().set_file_name(if html{"fourier.html"}else{"fourier.txt"}).save_file() else{return;};
        let result=(||->std::io::Result<()>{
            let mut w=BufWriter::new(std::fs::File::create(&path)?);
            if html{export_html(&mut w,d)?;}else{
                for i in 0..=d.paths.len()+1 {writeln!(w,"{}\n",manual(d,i))?;}
            }w.flush()
        })();
        self.status=match result{Ok(())=>format!("Saved {}. Open HTML in your browser.",path.display()),Err(e)=>e.to_string()};
    }
}
impl eframe::App for App{
    fn update(&mut self,ctx:&egui::Context,_:&mut eframe::Frame){
        if let Some(rx)=&self.worker{
            match rx.try_recv(){
                Ok(Ok(d))=>{self.status=format!("{} paths, {} coefficients{}",d.paths.len(),d.paths.iter().map(|p|p.coeff.len()).sum::<usize>(),if d.limited{" (limits reached)"}else{""});self.output=manual(&d,0);self.drawing=Some(d);self.worker=None;}
                Ok(Err(e))=>{self.status=e;self.worker=None;}
                Err(mpsc::TryRecvError::Disconnected)=>{self.status="Processing worker stopped".into();self.worker=None;}
                Err(mpsc::TryRecvError::Empty)=>{ctx.request_repaint_after(std::time::Duration::from_millis(100));}
            }
        }
        egui::SidePanel::left("controls").default_width(340.).show(ctx,|ui|{
            ui.heading("Fourier Generator");
            ui.add_enabled_ui(self.worker.is_none(),|ui|{
                let old=self.image_mode;
                ui.horizontal(|ui|{ui.radio_value(&mut self.image_mode,false,"Text");ui.radio_value(&mut self.image_mode,true,"Image");});
                if old!=self.image_mode{self.detail=if self.image_mode{300}else{150};}
                if self.image_mode{
                    if ui.button("Browse image").clicked(){self.file=rfd::FileDialog::new().add_filter("Images",&["png","jpg","jpeg","bmp","webp"]).pick_file();}
                    if let Some(p)=&self.file{ui.label(p.display().to_string());}
                    ui.checkbox(&mut self.color,"Color-aware tracing");
                    if self.color {ui.add(egui::Slider::new(&mut self.sensitivity,1..=100).text("Edge sensitivity"));}
                    ui.add(egui::Slider::new(&mut self.contours,5..=200).text("Max contours"));
                }else{ui.text_edit_singleline(&mut self.text);}
                ui.add(egui::Slider::new(&mut self.detail,50..=500).text("Max epicycles"));
                if ui.button("Calculate & Plot").clicked(){self.calculate(ctx.clone());}
            });
            ui.label(&self.status);ui.separator();
            if self.drawing.is_some(){
                if ui.button("Export Desmos HTML").clicked(){self.save(true);}
                if ui.button("Save manual equations").clicked(){self.save(false);}
                let d=self.drawing.as_ref().unwrap();let old=self.selected;
                let label=|i:usize|if i==0{"Instructions & Setup".to_string()}else if i>d.paths.len(){"Master Pen".to_string()}else{format!("Path {} ({})",i,d.paths[i-1].name)};
                egui::ComboBox::from_id_salt("paths").selected_text(label(self.selected)).show_ui(ui,|ui|{
                    for i in 0..=d.paths.len()+1{ui.selectable_value(&mut self.selected,i,label(i));}
                });
                if old!=self.selected{self.output=manual(d,self.selected);}
                if ui.button("Copy expression only").clicked(){ctx.copy_text(fourier_transformer::paste_expression(d,self.selected));}
                if self.selected>0 && self.selected<=d.paths.len(){
                    if ui.button("Copy upper t bound").clicked(){ctx.copy_text(fourier_transformer::path_bound(self.selected));}
                }
                // Avoid laying out multi-megabyte equations in the UI.
                let end=self.output.char_indices().nth(4000).map(|(i,_)|i).unwrap_or(self.output.len());
                egui::ScrollArea::vertical().show(ui,|ui|{ui.monospace(&self.output[..end]);if end<self.output.len(){ui.label("Preview truncated; Copy exports the complete equation.");}});
            }
        });
        egui::CentralPanel::default().show(ctx,|ui|{
            ui.heading("Outline preview");
            let (rect,_)=ui.allocate_exact_size(ui.available_size(),egui::Sense::hover());
            let Some(d)=&self.drawing else{return;};
            let mut minx=f32::INFINITY;let mut maxx=f32::NEG_INFINITY;let mut miny=f32::INFINITY;let mut maxy=f32::NEG_INFINITY;
            for p in &d.paths{for v in &p.preview{minx=minx.min(v.re);maxx=maxx.max(v.re);miny=miny.min(v.im);maxy=maxy.max(v.im);}}
            if !minx.is_finite(){return;}
            let scale=(rect.width()/(maxx-minx).max(0.01)).min(rect.height()/(maxy-miny).max(0.01))*0.9;
            let center=rect.center();let painter=ui.painter_at(rect);painter.rect_filled(rect,0.,egui::Color32::WHITE);
            for p in &d.paths{
                let points=p.preview.iter().map(|v|egui::pos2(center.x+(v.re-(minx+maxx)*0.5)*scale,center.y-(v.im-(miny+maxy)*0.5)*scale)).collect();
                painter.add(egui::Shape::line(points,egui::Stroke::new(1.5,egui::Color32::from_rgb(p.color[0],p.color[1],p.color[2]))));
            }
        });
    }
}
fn main()->eframe::Result<()>{
    eframe::run_native("Fourier Transformer",eframe::NativeOptions{viewport:egui::ViewportBuilder::default().with_inner_size([1150.,800.]),..Default::default()},Box::new(|_|Ok(Box::new(App::default()))))
}
