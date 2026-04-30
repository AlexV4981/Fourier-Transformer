import tkinter as tk
from tkinter import ttk, scrolledtext, messagebox
import numpy as np
import matplotlib.pyplot as plt
from matplotlib.textpath import TextPath
from matplotlib.backends.backend_tkagg import FigureCanvasTkAgg
import sys
import gc

class FourierDrawerStatic:
    def __init__(self, text):
        self.text = text
        self.points_per_char = 800
        self.max_epicycles_per_char = 150 
        
        self.char_data = []
        self.desmos_dict = {}
        self.desmos_keys = []
        
        self._process_text()
        self._generate_desmos_data()

    def _resample_path(self, complex_path):
        diffs = np.diff(complex_path)
        dists = np.abs(diffs)
        cum_dists = np.insert(np.cumsum(dists), 0, 0)
        
        if cum_dists[-1] == 0:
            return np.zeros(self.points_per_char, dtype=complex)
            
        t_target = np.linspace(0, cum_dists[-1], self.points_per_char, endpoint=False)
        x_interp = np.interp(t_target, cum_dists, np.real(complex_path))
        y_interp = np.interp(t_target, cum_dists, np.imag(complex_path))
        return x_interp + 1j * y_interp

    def _process_text(self):
        current_x = 0.0
        text_parsed = self.text.strip()
        text_parsed = text_parsed.replace("[heart]", "♥").replace("<3", "♥")
        text_parsed = text_parsed.replace("[square]", "■")
        text_parsed = text_parsed.replace("[star]", "★")
        text_parsed = text_parsed.replace("[note]", "♫")

        for char in text_parsed:
            if char == ' ':
                current_x += 1.5 
                continue
                
            tp = TextPath((0, 0), char, size=2)
            if len(tp.vertices) == 0:
                continue
                
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
        return epicycles[:self.max_epicycles_per_char]

    def _generate_desmos_data(self):
        """Generates dropdown entries including the Master Pen and Sequential Bounds."""
        N = len(self.char_data)
        
        instructions = f"--- GENERATED FOR {N} SYMBOLS ---\n\n"
        instructions += f"STEP 1: Create a slider 'a' in Desmos and set its bounds to: [0 <= a <= {N}].\n\n"
        instructions += "STEP 2: Use the dropdown above to copy each 'Path'.\n"
        instructions += "        When pasting a Path into Desmos, change its 't' bounds to the custom \\min\\max formula provided so it draws progressively.\n\n"
        instructions += "STEP 3: Select 'Master Pen' from the dropdown and copy it into its own box. This single equation uses piecewise logic to jump between letters automatically as 'a' increases!\n"
        
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
                
                # Equation for the static progressive line
                x_terms_t.append(f"{r:.3f}\\cos(2\\pi\\cdot{f}t + {p:.3f})")
                y_terms_t.append(f"{r:.3f}\\sin(2\\pi\\cdot{f}t + {p:.3f})")
                
                # Equation for the moving master pen (offset by letter index 'i')
                x_terms_a.append(f"{r:.3f}\\cos(2\\pi\\cdot{f}(a-{i}) + {p:.3f})")
                y_terms_a.append(f"{r:.3f}\\sin(2\\pi\\cdot{f}(a-{i}) + {p:.3f})")
                
            x_eq_t = " + ".join(x_terms_t)
            y_eq_t = " + ".join(y_terms_t)
            x_eq_a = " + ".join(x_terms_a)
            y_eq_a = " + ".join(y_terms_a)
            
            # Create Path Entry
            key_path = f"Path {i+1} ('{char}')"
            self.desmos_keys.append(key_path)
            
            path_str = f"({x_eq_t}, {y_eq_t})\n\n"
            path_str += "--- DESMOS BOUNDS ---\n"
            path_str += f"Change 't' bounds in Desmos to exactly this:\n"
            path_str += f"0 <= t <= \\min(1, \\max(0, a - {i}))"
            self.desmos_dict[key_path] = path_str
            
            # Append Piecewise Logic for Master Pen
            if i < N - 1:
                pen_x_pieces.append(f"{i}<=a<{i+1}: {x_eq_a}")
                pen_y_pieces.append(f"{i}<=a<{i+1}: {y_eq_a}")
            else:
                pen_x_pieces.append(f"{i}<=a<={i+1}: {x_eq_a}")
                pen_y_pieces.append(f"{i}<=a<={i+1}: {y_eq_a}")

        # Create Master Pen Entry
        master_pen_x = "\\{" + ", ".join(pen_x_pieces) + "\\}"
        master_pen_y = "\\{" + ", ".join(pen_y_pieces) + "\\}"
        
        key_pen = "Master Pen (Single Dot)"
        self.desmos_keys.append(key_pen)
        self.desmos_dict[key_pen] = f"({master_pen_x}, {master_pen_y})"


class FourierApp:
    def __init__(self, root):
        self.root = root
        self.root.title("Fourier Text Generator (Streamlined Copy)")
        self.root.geometry("1150x750")
        self.drawer = None
        self._build_gui()
        
    def _build_gui(self):
        control_frame = ttk.Frame(self.root, padding="10", width=350)
        control_frame.pack(side=tk.LEFT, fill=tk.Y)
        
        ttk.Label(control_frame, text="Input Text (try 'I [heart] MATH'):", font=("Arial", 10, "bold")).pack(anchor=tk.W, pady=(0, 5))
        self.entry_text = ttk.Entry(control_frame, width=35, font=("Arial", 12))
        self.entry_text.insert(0, "I [heart] MATH")
        self.entry_text.pack(fill=tk.X, pady=(0, 15))
        
        self.btn_calc = ttk.Button(control_frame, text="Calculate & Plot", command=self.calculate_math)
        self.btn_calc.pack(fill=tk.X, pady=5, ipady=8)
        
        # --- NEW DROPDOWN & COPY SECTION ---
        copy_frame = ttk.LabelFrame(control_frame, text=" Desmos Export ", padding="10")
        copy_frame.pack(fill=tk.BOTH, expand=True, pady=(20, 5))
        
        ttk.Label(copy_frame, text="Select Item to View/Copy:").pack(anchor=tk.W, pady=(0, 5))
        
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

    def calculate_math(self):
        user_input = self.entry_text.get()
        if not user_input.strip():
            messagebox.showwarning("Warning", "Please enter some text!")
            return
            
        self.btn_calc.config(text="Calculating Math...")
        self.root.update()
        
        self.ax.clear()
        gc.collect()

        try:
            self.drawer = FourierDrawerStatic(user_input)
        except Exception as e:
            messagebox.showerror("Error", f"Could not calculate Fourier Transform:\n{e}")
            self.btn_calc.config(text="Calculate & Plot")
            return

        self.ax.set_aspect('equal')
        self.ax.axis('off')
        self.ax.set_title(f"Preview: '{user_input}'\n(Dropdown populated for easy copying)")

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
        
        # Populate UI Dropdown
        self.combo_box['values'] = self.drawer.desmos_keys
        self.combo_box.current(0)  # Select first item (Instructions)
        self.btn_copy.config(state="normal")
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
            
            # Flash the button to confirm copy
            original_text = self.btn_copy.cget("text")
            self.btn_copy.config(text="✅ Copied!")
            self.root.after(1000, lambda: self.btn_copy.config(text=original_text))

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