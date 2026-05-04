import tkinter as tk
from tkinter import ttk, scrolledtext, messagebox, filedialog
import numpy as np
import matplotlib.pyplot as plt
from matplotlib.textpath import TextPath
from matplotlib.backends.backend_tkagg import FigureCanvasTkAgg
import sys
import gc
import webbrowser
import tempfile
import os

try:
    import cv2
except ImportError:
    messagebox.showerror("Missing Module", "Please install OpenCV to use Image Mode:\npip install opencv-python")
    sys.exit()

class FourierDrawer:
    def __init__(self, input_data, mode='text', max_epicycles=150):
        self.input_data = input_data
        self.mode = mode
        self.points_per_path = 1500  # Increased for images
        self.max_epicycles = max_epicycles
        
        self.char_data = []
        self.desmos_dict = {}
        self.desmos_keys = []
        
        if self.mode == 'text':
            self._process_text()
        elif self.mode == 'image':
            self._process_image()
            
        self._generate_desmos_data()

    def _resample_path(self, complex_path):
        diffs = np.diff(complex_path)
        dists = np.abs(diffs)
        cum_dists = np.insert(np.cumsum(dists), 0, 0)
        
        if cum_dists[-1] == 0:
            return np.zeros(self.points_per_path, dtype=complex)
            
        t_target = np.linspace(0, cum_dists[-1], self.points_per_path, endpoint=False)
        x_interp = np.interp(t_target, cum_dists, np.real(complex_path))
        y_interp = np.interp(t_target, cum_dists, np.imag(complex_path))
        return x_interp + 1j * y_interp

    def _compute_dft(self, path):
        N = len(path)
        epicycles = []
        for n in range(-N//2 + 1, N//2 + 1):
            c_n = np.mean(path * np.exp(-1j * 2 * np.pi * n * np.arange(N) / N))
            epicycles.append({
                'freq': n,
                'amp': np.abs(c_n),
                'phase': np.angle(c_n)
            })
        epicycles.sort(key=lambda x: x['amp'], reverse=True)
        return epicycles[:self.max_epicycles]

    def _process_text(self):
        current_x = 0.0
        text_parsed = self.input_data.strip()
        text_parsed = text_parsed.replace("[heart]", "♥").replace("<3", "♥")
        text_parsed = text_parsed.replace("[square]", "■")
        text_parsed = text_parsed.replace("[star]", "★")
        text_parsed = text_parsed.replace("[note]", "♫")

        for char in text_parsed:
            if char == ' ':
                current_x += 1.5 
                continue
                
            tp = TextPath((0, 0), char, size=2)
            if len(tp.vertices) == 0: continue
                
            vertices = tp.vertices.copy()
            vertices[:, 1] -= (vertices[:, 1].max() + vertices[:, 1].min()) / 2
            
            min_x = vertices[:, 0].min()
            vertices[:, 0] -= min_x
            vertices[:, 0] += current_x
            
            char_width = vertices[:, 0].max() - vertices[:, 0].min()
            current_x += char_width + max(0.3, char_width * 0.2)
            
            path_complex = vertices[:, 0] + 1j * vertices[:, 1]
            resampled_path = self._resample_path(path_complex)
            epicycles = self._compute_dft(resampled_path)
            
            self.char_data.append({
                'char': char,
                'path': resampled_path,
                'epicycles': epicycles
            })

    def _process_image(self):
        """Processes EVERY internal and external line in the image."""
        img = cv2.imread(self.input_data, cv2.IMREAD_GRAYSCALE)
        if img is None:
            raise ValueError("Could not load image.")

        # 1. Improved Pre-processing
        # We use a slight blur to join tiny gaps in the sketch lines
        blurred = cv2.GaussianBlur(img, (3, 3), 0)
        
        # 2. Adaptive Thresholding 
        # This is CRITICAL for sketches to pick up the fine lines in the center
        thresh = cv2.adaptiveThreshold(blurred, 255, cv2.ADAPTIVE_THRESH_GAUSSIAN_C, 
                                     cv2.THRESH_BINARY_INV, 11, 2)
        
        # 3. RETR_LIST captures ALL contours, including the ones inside others
        contours, _ = cv2.findContours(thresh, cv2.RETR_LIST, cv2.CHAIN_APPROX_SIMPLE)
        
        if not contours:
            raise ValueError("No outlines found.")

        # 4. Filter out noise
        # Increase this number (e.g., to 10 or 15) if you get too many tiny dots
        valid_contours = [cnt for cnt in contours if len(cnt) > 8]
        
        # Sort by size (largest lines first)
        valid_contours.sort(key=len, reverse=True)

        # Calculate global scaling based on the overall image dimensions
        # This keeps the flower centers in the correct spot relative to the petals
        h, w = img.shape
        center_x, center_y = w / 2, h / 2
        max_dim = max(h, w)

        for i, cnt in enumerate(valid_contours):
            pts = cnt.reshape(-1, 2)
            
            # Convert to complex and normalize relative to the image center
            path_complex = (pts[:, 0] - center_x) - 1j * (pts[:, 1] - center_y)
            
            # Scale so the whole image fits in a Desmos radius of 5
            path_complex = path_complex / (max_dim / 10)

            resampled_path = self._resample_path(path_complex)
            epicycles = self._compute_dft(resampled_path)
            
            self.char_data.append({
                'char': f"Line {i+1}",
                'path': resampled_path,
                'epicycles': epicycles
            })

    def _generate_desmos_data(self):
        N = len(self.char_data)
        
        instructions = f"--- GENERATED FOR {N} PATHS ---\n\n"
        instructions += f"STEP 1: Create a slider 'a' in Desmos and set its bounds to: [0 <= a <= {N}].\n\n"
        instructions += "STEP 2: Use the dropdown above to copy each 'Path'.\n"
        instructions += "        When pasting a Path into Desmos, change its 't' bounds to the custom \\min\\max formula provided so it draws progressively.\n\n"
        instructions += "STEP 3: Select 'Master Pen' from the dropdown and copy it into its own box. This single equation uses piecewise logic to jump between paths automatically as 'a' increases!\n"
        
        self.desmos_keys.append("Instructions & Setup")
        self.desmos_dict["Instructions & Setup"] = instructions
        
        pen_x_pieces = []
        pen_y_pieces = []
        
        for i, data in enumerate(self.char_data):
            char = data['char']
            epicycles = data['epicycles']
            
            x_terms_t, y_terms_t = [], []
            x_terms_a, y_terms_a = [], []
            
            for epi in epicycles:
                r, f, p = epi['amp'], epi['freq'], epi['phase']
                if r < 0.001: continue
                
                x_terms_t.append(f"{r:.3f}\\cos(2\\pi\\cdot{f}t + {p:.3f})")
                y_terms_t.append(f"{r:.3f}\\sin(2\\pi\\cdot{f}t + {p:.3f})")
                
                x_terms_a.append(f"{r:.3f}\\cos(2\\pi\\cdot{f}(a-{i}) + {p:.3f})")
                y_terms_a.append(f"{r:.3f}\\sin(2\\pi\\cdot{f}(a-{i}) + {p:.3f})")
                
            x_eq_t = " + ".join(x_terms_t) if x_terms_t else "0"
            y_eq_t = " + ".join(y_terms_t) if y_terms_t else "0"
            x_eq_a = " + ".join(x_terms_a) if x_terms_a else "0"
            y_eq_a = " + ".join(y_terms_a) if y_terms_a else "0"
            
            key_path = f"Path {i+1} ('{char}')"
            self.desmos_keys.append(key_path)
            
            path_str = f"({x_eq_t}, {y_eq_t})\n\n"
            path_str += "--- DESMOS BOUNDS ---\n"
            path_str += f"Change 't' bounds in Desmos to exactly this:\n"
            path_str += f"0 <= t <= \\min(1, \\max(0, a - {i}))"
            self.desmos_dict[key_path] = path_str
            
            if i < N - 1:
                pen_x_pieces.append(f"{i}<=a<{i+1}: {x_eq_a}")
                pen_y_pieces.append(f"{i}<=a<{i+1}: {y_eq_a}")
            else:
                pen_x_pieces.append(f"{i}<=a<={i+1}: {x_eq_a}")
                pen_y_pieces.append(f"{i}<=a<={i+1}: {y_eq_a}")

        master_pen_x = "\\{" + ", ".join(pen_x_pieces) + "\\}"
        master_pen_y = "\\{" + ", ".join(pen_y_pieces) + "\\}"
        
        key_pen = "Master Pen (Single Dot)"
        self.desmos_keys.append(key_pen)
        self.desmos_dict[key_pen] = f"({master_pen_x}, {master_pen_y})"


class FourierApp:
    def __init__(self, root):
        self.root = root
        self.root.title("Fourier Generator: Text & Images")
        self.root.geometry("1150x800")
        self.drawer = None
        self.image_path = ""
        self._build_gui()
        
    def _build_gui(self):
        control_frame = ttk.Frame(self.root, padding="10", width=350)
        control_frame.pack(side=tk.LEFT, fill=tk.Y)
        
        # --- MODE SELECTION ---
        self.mode_var = tk.StringVar(value="text")
        
        ttk.Label(control_frame, text="Input Mode:", font=("Arial", 10, "bold")).pack(anchor=tk.W, pady=(0, 5))
        radio_frame = ttk.Frame(control_frame)
        radio_frame.pack(fill=tk.X, pady=(0, 10))
        ttk.Radiobutton(radio_frame, text="Text", variable=self.mode_var, value="text", command=self.toggle_mode).pack(side=tk.LEFT, padx=(0, 10))
        ttk.Radiobutton(radio_frame, text="Image (Silhouette)", variable=self.mode_var, value="image", command=self.toggle_mode).pack(side=tk.LEFT)
        
        # --- INPUT FIELDS ---
        self.input_frame = ttk.Frame(control_frame)
        self.input_frame.pack(fill=tk.X, pady=(0, 15))
        
        self.lbl_input = ttk.Label(self.input_frame, text="Input Text:")
        self.lbl_input.pack(anchor=tk.W)
        
        self.entry_text = ttk.Entry(self.input_frame, width=35, font=("Arial", 12))
        self.entry_text.insert(0, "I [heart] MATH")
        self.entry_text.pack(fill=tk.X, pady=2)
        
        self.btn_browse = ttk.Button(self.input_frame, text="Browse Image...", command=self.browse_image)
        # Browse button hidden by default
        
        # --- DETAIL SLIDER ---
        ttk.Label(control_frame, text="Detail (Max Epicycles per Path):").pack(anchor=tk.W, pady=(10, 0))
        self.detail_var = tk.IntVar(value=150)
        self.slider_detail = ttk.Scale(control_frame, from_=50, to=500, variable=self.detail_var, orient=tk.HORIZONTAL)
        self.slider_detail.pack(fill=tk.X, pady=(5, 5))
        self.lbl_detail_val = ttk.Label(control_frame, textvariable=self.detail_var)
        self.lbl_detail_val.pack(anchor=tk.W, pady=(0, 10))
        
        self.btn_calc = ttk.Button(control_frame, text="Calculate & Plot", command=self.calculate_math)
        self.btn_calc.pack(fill=tk.X, pady=5, ipady=8)
        
        # --- EXPORT SECTION ---
        copy_frame = ttk.LabelFrame(control_frame, text=" Desmos Export ", padding="10")
        copy_frame.pack(fill=tk.BOTH, expand=True, pady=(20, 5))
        
        self.btn_auto_desmos = ttk.Button(copy_frame, text="🌐 Auto-Draw in Desmos", command=self.auto_draw_desmos, state="disabled")
        self.btn_auto_desmos.pack(fill=tk.X, ipady=8, pady=(0, 15))
        
        ttk.Separator(copy_frame, orient='horizontal').pack(fill=tk.X, pady=(0, 15))
        ttk.Label(copy_frame, text="Manual Copy (Optional):").pack(anchor=tk.W, pady=(0, 5))
        
        self.combo_var = tk.StringVar()
        self.combo_box = ttk.Combobox(copy_frame, textvariable=self.combo_var, state="readonly")
        self.combo_box.pack(fill=tk.X, pady=(0, 10))
        self.combo_box.bind("<<ComboboxSelected>>", self.on_combo_select)
        
        self.btn_copy = ttk.Button(copy_frame, text="📋 Copy to Clipboard", command=self.copy_to_clipboard, state="disabled")
        self.btn_copy.pack(fill=tk.X, ipady=5, pady=(0, 10))
        
        self.txt_desmos = scrolledtext.ScrolledText(copy_frame, wrap=tk.WORD, font=("Courier", 9))
        self.txt_desmos.pack(fill=tk.BOTH, expand=True)

        # --- PLOT SECTION ---
        plot_frame = ttk.Frame(self.root)
        plot_frame.pack(side=tk.RIGHT, fill=tk.BOTH, expand=True)
        
        self.fig, self.ax = plt.subplots(figsize=(8, 8), dpi=100)
        self.canvas = FigureCanvasTkAgg(self.fig, master=plot_frame)
        self.canvas.get_tk_widget().pack(fill=tk.BOTH, expand=True)

    def toggle_mode(self):
        mode = self.mode_var.get()
        if mode == "text":
            self.btn_browse.pack_forget()
            self.lbl_input.config(text="Input Text:")
            self.entry_text.pack(fill=tk.X, pady=2)
            self.entry_text.config(state="normal")
            self.detail_var.set(150) # Standard detail for text
        else:
            self.entry_text.pack_forget()
            self.lbl_input.config(text="Selected Image:")
            self.btn_browse.pack(fill=tk.X, pady=2)
            self.entry_text.pack(fill=tk.X, pady=2)
            self.entry_text.delete(0, tk.END)
            self.entry_text.insert(0, self.image_path if self.image_path else "No file selected")
            self.entry_text.config(state="readonly")
            self.detail_var.set(300) # Higher detail needed for images

    def browse_image(self):
        filepath = filedialog.askopenfilename(
            title="Select a Silhouette Image",
            filetypes=[("Image Files", "*.png *.jpg *.jpeg *.bmp")]
        )
        if filepath:
            self.image_path = filepath
            self.entry_text.config(state="normal")
            self.entry_text.delete(0, tk.END)
            self.entry_text.insert(0, filepath)
            self.entry_text.config(state="readonly")

    def calculate_math(self):
        mode = self.mode_var.get()
        
        if mode == "text":
            input_data = self.entry_text.get()
            if not input_data.strip():
                messagebox.showwarning("Warning", "Please enter some text!")
                return
        else:
            input_data = self.image_path
            if not input_data or not os.path.exists(input_data):
                messagebox.showwarning("Warning", "Please select a valid image file!")
                return
            
        self.btn_calc.config(text="Calculating Math...")
        self.root.update()
        
        self.ax.clear()
        gc.collect()

        try:
            self.drawer = FourierDrawer(
                input_data=input_data, 
                mode=mode, 
                max_epicycles=self.detail_var.get()
            )
        except Exception as e:
            messagebox.showerror("Error", f"Processing failed:\n{e}")
            self.btn_calc.config(text="Calculate & Plot")
            return

        self.ax.set_aspect('equal')
        self.ax.axis('off')
        title = f"Preview: '{input_data}'" if mode == "text" else f"Preview: {os.path.basename(input_data)}"
        self.ax.set_title(title)

        all_x, all_y = [], []
        for data in self.drawer.char_data:
            path = data['path']
            x, y = np.real(path), np.imag(path)
            all_x.extend(x)
            all_y.extend(y)
            self.ax.plot(x, y, 'b-', linewidth=1.5, antialiased=True)

        if all_x and all_y:
            max_x, min_x = max(all_x), min(all_x)
            max_y, min_y = max(all_y), min(all_y)
            margin_x = (max_x - min_x) * 0.1
            margin_y = (max_y - min_y) * 0.1
            
            range_val = max((max_x - min_x), (max_y - min_y)) / 2
            mid_x = (max_x + min_x) / 2
            mid_y = (max_y + min_y) / 2
            
            self.ax.set_xlim(mid_x - range_val - margin_x, mid_x + range_val + margin_x)
            self.ax.set_ylim(mid_y - range_val - margin_y, mid_y + range_val + margin_y)

        self.canvas.draw()
        
        # Enable UI
        self.combo_box['values'] = self.drawer.desmos_keys
        self.combo_box.current(0)
        self.btn_copy.config(state="normal")
        self.btn_auto_desmos.config(state="normal")
        self.on_combo_select(None)
        
        self.btn_calc.config(text="Calculate & Plot")

    def on_combo_select(self, event):
        selected_key = self.combo_var.get()
        if not self.drawer or selected_key not in self.drawer.desmos_dict:
            return
            
        content = self.drawer.desmos_dict[selected_key]
        self.txt_desmos.delete(1.0, tk.END)
        self.txt_desmos.insert(tk.END, content)

    def copy_to_clipboard(self):
        content = self.txt_desmos.get(1.0, tk.END).strip()
        if content:
            self.root.clipboard_clear()
            self.root.clipboard_append(content)
            
            original_text = self.btn_copy.cget("text")
            self.btn_copy.config(text="✅ Copied!")
            self.root.after(1000, lambda: self.btn_copy.config(text=original_text))

    def auto_draw_desmos(self):
        if not self.drawer: return
            
        N = len(self.drawer.char_data)
        
        html_lines = [
            "<!DOCTYPE html>",
            "<html><head><meta charset='utf-8'>",
            "<title>Fourier Drawing</title>",
            "<script src='https://www.desmos.com/api/v1.8/calculator.js?apiKey=dcb31709b452b1cf9dc26972add0fda6'></script>",
            "<style>body, html {margin: 0; padding: 0; width: 100%; height: 100%; overflow: hidden;} #calculator {width: 100%; height: 100%;}</style>",
            "</head><body>",
            "<div id='calculator'></div>",
            "<script>",
            "var elt = document.getElementById('calculator');",
            "var calc = Desmos.GraphingCalculator(elt);",
            f"calc.setExpression({{id: 'slider_a', latex: 'a=0', sliderBounds: {{min: 0, max: {N}, step: 0.01}}}});"
        ]
        
        pen_x_pieces, pen_y_pieces = [], []
        
        for i, data in enumerate(self.drawer.char_data):
            epicycles = data['epicycles']
            x_terms_t, y_terms_t = [], []
            x_terms_a, y_terms_a = [], []
            
            for epi in epicycles:
                r, f, p = epi['amp'], epi['freq'], epi['phase']
                if r < 0.001: continue
                
                x_terms_t.append(f"{r:.3f}\\\\cos(2\\\\pi\\\\cdot{f}t + {p:.3f})")
                y_terms_t.append(f"{r:.3f}\\\\sin(2\\\\pi\\\\cdot{f}t + {p:.3f})")
                
                x_terms_a.append(f"{r:.3f}\\\\cos(2\\\\pi\\\\cdot{f}(a-{i}) + {p:.3f})")
                y_terms_a.append(f"{r:.3f}\\\\sin(2\\\\pi\\\\cdot{f}(a-{i}) + {p:.3f})")
                
            x_eq_t = " + ".join(x_terms_t) if x_terms_t else "0"
            y_eq_t = " + ".join(y_terms_t) if y_terms_t else "0"
            x_eq_a = " + ".join(x_terms_a) if x_terms_a else "0"
            y_eq_a = " + ".join(y_terms_a) if y_terms_a else "0"
            
            path_latex = f"({x_eq_t}, {y_eq_t})"
            bounds = f"\\\\min(1, \\\\max(0, a-{i}))"
            html_lines.append(f"calc.setExpression({{id: 'path_{i}', latex: '{path_latex}', parametricDomain: {{min: '0', max: '{bounds}'}}, color: Desmos.Colors.BLUE}});")
            
            if i < N - 1:
                pen_x_pieces.append(f"{i}<=a<{i+1}: {x_eq_a}")
                pen_y_pieces.append(f"{i}<=a<{i+1}: {y_eq_a}")
            else:
                pen_x_pieces.append(f"{i}<=a<={i+1}: {x_eq_a}")
                pen_y_pieces.append(f"{i}<=a<={i+1}: {y_eq_a}")

        master_x = "\\\\{" + ", ".join(pen_x_pieces) + "\\\\}"
        master_y = "\\\\{" + ", ".join(pen_y_pieces) + "\\\\}"
        master_latex = f"({master_x}, {master_y})"
        
        html_lines.append(f"calc.setExpression({{id: 'pen', latex: '{master_latex}', color: Desmos.Colors.RED, pointStyle: Desmos.Styles.POINT}});")
        html_lines.append("</script></body></html>")
        
        fd, temp_path = tempfile.mkstemp(suffix=".html", prefix="desmos_fourier_")
        with os.fdopen(fd, 'w', encoding='utf-8') as f:
            f.write("\n".join(html_lines))
            
        webbrowser.open('file://' + os.path.realpath(temp_path))


if __name__ == "__main__":
    if sys.platform == "win32":
        try:
            import ctypes
            ctypes.windll.shcore.SetProcessDpiAwareness(1)
        except Exception:
            pass

    root = tk.Tk()
    app = FourierApp(root)
    root.mainloop()
