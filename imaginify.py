"""
Imaginify - Modern desktop image tool (dark mode, CustomTkinter)
Resize, crop, compress and convert images with the best quality settings.

Made by JezPress  -  https://jezpress.com/

Requirements:
    pip install Pillow customtkinter
"""

import os
import sys
import threading
from pathlib import Path

__version__ = "1.0.1"
APP_NAME = "Imaginify"
APP_AUTHOR = "JezPress"

try:
    import customtkinter as ctk
except ImportError:
    print("CustomTkinter is required. Install it with:  pip install customtkinter")
    sys.exit(1)

try:
    from PIL import Image, ImageTk, ImageOps
except ImportError:
    print("Pillow is required. Install it with:  pip install Pillow")
    sys.exit(1)

import tkinter as tk
from tkinter import filedialog, messagebox

try:
    from tkinterdnd2 import TkinterDnD, DND_FILES
    DND_AVAILABLE = True
except ImportError:
    DND_AVAILABLE = False


class CTkDnD(ctk.CTk):
    """CTk root with tkinterdnd2 wired in so widgets can accept OS-level file drops."""
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        if DND_AVAILABLE:
            self.TkdndVersion = TkinterDnD._require(self)

SUPPORTED_EXTS = {".jpg", ".jpeg", ".png", ".webp", ".bmp", ".tif", ".tiff", ".gif"}

ctk.set_appearance_mode("dark")
ctk.set_default_color_theme("blue")

ACCENT = "#3b82f6"
ACCENT_HOVER = "#2563eb"
SUCCESS = "#22c55e"
SUCCESS_HOVER = "#16a34a"
CANVAS_BG = "#1a1d23"
CARD_BG = "#23272f"
LIST_BG = "#1f232a"
LIST_HOVER = "#2a2f38"
LIST_SELECTED = "#2b3a55"
TEXT_DIM = "#9ca3af"
CROP_COLOR = "#fbbf24"

PRESETS = [
    ("1920w (HD)",       1920, 0),
    ("1280w (web)",      1280, 0),
    ("800w (thumb)",      800, 0),
    ("Instagram 1080",   1080, 1080),
    ("Facebook 2048w",   2048, 0),
    ("1080p HD",         1920, 1080),
]


def open_image(path):
    img = Image.open(path)
    try:
        img = ImageOps.exif_transpose(img)
    except Exception:
        pass
    return img


def resize_image(img, width, height, keep_aspect):
    if not width and not height:
        return img
    orig_w, orig_h = img.size
    if keep_aspect:
        if width and not height:
            ratio = width / orig_w
            height = max(1, int(round(orig_h * ratio)))
        elif height and not width:
            ratio = height / orig_h
            width = max(1, int(round(orig_w * ratio)))
        else:
            ratio = min(width / orig_w, height / orig_h)
            width = max(1, int(round(orig_w * ratio)))
            height = max(1, int(round(orig_h * ratio)))
    else:
        width = width or orig_w
        height = height or orig_h
    return img.resize((width, height), Image.Resampling.LANCZOS)


def crop_image(img, box):
    left, top, right, bottom = box
    left = max(0, int(left))
    top = max(0, int(top))
    right = min(img.width, int(right))
    bottom = min(img.height, int(bottom))
    if right - left < 1 or bottom - top < 1:
        return img
    return img.crop((left, top, right, bottom))


def save_image(img, out_path, fmt, quality):
    fmt = fmt.upper()
    save_kwargs = {}
    if fmt in ("JPG", "JPEG"):
        fmt = "JPEG"
        if img.mode in ("RGBA", "LA", "P"):
            background = Image.new("RGB", img.size, (255, 255, 255))
            if img.mode == "P":
                img = img.convert("RGBA")
            mask = img.split()[-1] if img.mode in ("RGBA", "LA") else None
            background.paste(img, mask=mask)
            img = background
        elif img.mode != "RGB":
            img = img.convert("RGB")
        save_kwargs = dict(
            quality=quality, optimize=True, progressive=True,
            subsampling=0 if quality >= 90 else 2,
        )
    elif fmt == "PNG":
        save_kwargs = dict(optimize=True, compress_level=9)
    elif fmt == "WEBP":
        save_kwargs = dict(quality=quality, method=6)
        if quality >= 100:
            save_kwargs["lossless"] = True
    img.save(out_path, format=fmt, **save_kwargs)


def _human(n):
    if n is None:
        return "-"
    n = float(n)
    for unit in ("B", "KB", "MB", "GB"):
        if abs(n) < 1024:
            return f"{n:.1f} {unit}"
        n /= 1024
    return f"{n:.1f} TB"


class FileRow(ctk.CTkFrame):
    """One file row with a checkbox AND a click-to-preview area."""

    def __init__(self, parent, name, index, on_click, on_check, checked):
        super().__init__(parent, fg_color=LIST_BG, corner_radius=6, height=34)
        self.index = index
        self.on_click = on_click
        self.on_check = on_check
        self.selected = False
        self.check_var = tk.BooleanVar(value=checked)
        self.checkbox = ctk.CTkCheckBox(
            self, text="", variable=self.check_var, width=20,
            checkbox_width=18, checkbox_height=18, corner_radius=4,
            fg_color=ACCENT, hover_color=ACCENT_HOVER,
            command=self._on_check_changed,
        )
        self.checkbox.pack(side="left", padx=(8, 0), pady=4)
        self.label = ctk.CTkLabel(self, text=name, anchor="w",
                                  font=ctk.CTkFont(size=12))
        self.label.pack(side="left", fill="both", expand=True, padx=(6, 10), pady=4)
        for widget in (self, self.label):
            widget.bind("<Button-1>", self._handle_click)
            widget.bind("<Enter>", self._on_enter)
            widget.bind("<Leave>", self._on_leave)

    def _handle_click(self, _e):
        self.on_click(self.index)

    def _on_check_changed(self):
        self.on_check(self.index, self.check_var.get())

    def _on_enter(self, _e):
        if not self.selected:
            self.configure(fg_color=LIST_HOVER)

    def _on_leave(self, _e):
        if not self.selected:
            self.configure(fg_color=LIST_BG)

    def set_selected(self, sel):
        self.selected = sel
        self.configure(fg_color=LIST_SELECTED if sel else LIST_BG)

    def set_checked(self, val):
        self.check_var.set(val)


class Imaginify:
    def __init__(self, root):
        self.root = root
        root.title(f"{APP_NAME} v{__version__}  -  by {APP_AUTHOR}")
        root.geometry("1200x800")
        root.minsize(1020, 700)

        self.files = []
        self.checked_paths = set()
        self.file_rows = []
        self.selected_index = None
        self.current_img = None
        self.preview_img = None
        self.preview_tk = None
        self.preview_scale = 1.0
        self.preview_offset = (0, 0)

        self.crop_start = None
        self.crop_rect_id = None
        self.crop_box_preview = None

        self.width_var = tk.StringVar()
        self.height_var = tk.StringVar()
        self.aspect_var = tk.BooleanVar(value=True)
        self.quality_var = tk.IntVar(value=88)
        self.format_var = tk.StringVar(value="Keep original")
        self.output_var = tk.StringVar(
            value=str(Path.home() / "Pictures" / "Imaginify"))
        self.suffix_var = tk.StringVar(value="_optimised")

        self._build_ui()
        self._register_drop_targets()

    def _register_drop_targets(self):
        """Enable OS-level drag-and-drop from Finder / Explorer onto the window."""
        if not DND_AVAILABLE:
            return
        for target in (self.root, self.file_list_frame, self.empty_label, self.canvas):
            try:
                target.drop_target_register(DND_FILES)
                target.dnd_bind("<<Drop>>", self._on_drop)
            except Exception:
                pass

    def _on_drop(self, event):
        try:
            raw = self.root.tk.splitlist(event.data)
        except Exception:
            raw = [event.data]
        paths = []
        for entry in raw:
            entry = entry.strip().strip("{}")
            if not entry:
                continue
            path = Path(entry)
            if path.is_dir():
                for p in path.rglob("*"):
                    if p.is_file() and p.suffix.lower() in SUPPORTED_EXTS:
                        paths.append(str(p))
            elif path.is_file() and path.suffix.lower() in SUPPORTED_EXTS:
                paths.append(str(path))
        if paths:
            self._add_paths(paths)
        else:
            self.status.configure(text="Nothing added - dropped items are not supported images.")

    def _build_ui(self):
        self.root.grid_columnconfigure(0, weight=1)
        self.root.grid_rowconfigure(1, weight=1)

        header = ctk.CTkFrame(self.root, fg_color="transparent")
        header.grid(row=0, column=0, sticky="ew", padx=16, pady=(14, 6))
        header.grid_columnconfigure(5, weight=1)

        ctk.CTkLabel(header, text="Imaginify",
                     font=ctk.CTkFont(size=22, weight="bold")
                     ).grid(row=0, column=0, sticky="w")
        ctk.CTkLabel(header,
                     text=f"Resize - Crop - Compress - Convert   |   "
                          f"v{__version__} by {APP_AUTHOR}",
                     font=ctk.CTkFont(size=12), text_color=TEXT_DIM
                     ).grid(row=0, column=1, sticky="w", padx=(10, 0))

        ctk.CTkButton(header, text="Add files", width=110,
                      command=self.add_files,
                      fg_color=ACCENT, hover_color=ACCENT_HOVER
                      ).grid(row=0, column=2, padx=(20, 6))
        ctk.CTkButton(header, text="Add folder", width=110,
                      command=self.add_folder,
                      fg_color=ACCENT, hover_color=ACCENT_HOVER
                      ).grid(row=0, column=3, padx=6)
        ctk.CTkButton(header, text="Remove", width=90,
                      command=self.remove_current,
                      fg_color="transparent", border_width=1,
                      border_color=TEXT_DIM, hover_color=LIST_HOVER
                      ).grid(row=0, column=4, padx=6)
        ctk.CTkButton(header, text="Clear all", width=90,
                      command=self.clear_files,
                      fg_color="transparent", border_width=1,
                      border_color=TEXT_DIM, hover_color=LIST_HOVER
                      ).grid(row=0, column=6, padx=6, sticky="e")

        body = ctk.CTkFrame(self.root, fg_color="transparent")
        body.grid(row=1, column=0, sticky="nsew", padx=16, pady=6)
        body.grid_columnconfigure(1, weight=1)
        body.grid_rowconfigure(0, weight=1)

        left_card = ctk.CTkFrame(body, fg_color=CARD_BG, corner_radius=10, width=300)
        left_card.grid(row=0, column=0, sticky="ns")
        left_card.grid_propagate(False)
        left_card.grid_rowconfigure(2, weight=1)
        left_card.grid_columnconfigure(0, weight=1)

        list_header = ctk.CTkFrame(left_card, fg_color="transparent")
        list_header.grid(row=0, column=0, sticky="ew", padx=14, pady=(12, 4))
        list_header.grid_columnconfigure(1, weight=1)
        ctk.CTkLabel(list_header, text="Files",
                     font=ctk.CTkFont(size=13, weight="bold")
                     ).grid(row=0, column=0, sticky="w")
        self.list_count = ctk.CTkLabel(
            list_header, text="0 of 0 selected",
            text_color=TEXT_DIM, font=ctk.CTkFont(size=11))
        self.list_count.grid(row=0, column=1, sticky="e")

        select_row = ctk.CTkFrame(left_card, fg_color="transparent")
        select_row.grid(row=1, column=0, sticky="ew", padx=14, pady=(0, 4))
        ctk.CTkButton(select_row, text="Select all", width=90, height=26,
                      command=self.select_all,
                      fg_color="transparent", border_width=1,
                      border_color=TEXT_DIM, hover_color=LIST_HOVER,
                      font=ctk.CTkFont(size=11)
                      ).pack(side="left", padx=(0, 6))
        ctk.CTkButton(select_row, text="Select none", width=90, height=26,
                      command=self.select_none,
                      fg_color="transparent", border_width=1,
                      border_color=TEXT_DIM, hover_color=LIST_HOVER,
                      font=ctk.CTkFont(size=11)
                      ).pack(side="left")

        self.file_list_frame = ctk.CTkScrollableFrame(left_card, fg_color="transparent")
        self.file_list_frame.grid(row=2, column=0, sticky="nsew", padx=8, pady=(4, 10))
        empty_hint = ('No files yet.\nDrag images or folders here,\nor click "Add files" / "Add folder".'
                      if DND_AVAILABLE else
                      'No files yet.\nClick "Add files" or "Add folder" to start.')
        self.empty_label = ctk.CTkLabel(
            self.file_list_frame,
            text=empty_hint,
            text_color=TEXT_DIM, font=ctk.CTkFont(size=11), justify="center")
        self.empty_label.pack(pady=30)

        right_card = ctk.CTkFrame(body, fg_color=CARD_BG, corner_radius=10)
        right_card.grid(row=0, column=1, sticky="nsew", padx=(12, 0))
        right_card.grid_rowconfigure(1, weight=1)
        right_card.grid_columnconfigure(0, weight=1)

        preview_header = ctk.CTkFrame(right_card, fg_color="transparent")
        preview_header.grid(row=0, column=0, sticky="ew", padx=14, pady=(10, 6))
        preview_header.grid_columnconfigure(2, weight=1)
        ctk.CTkLabel(preview_header, text="Preview",
                     font=ctk.CTkFont(size=13, weight="bold")
                     ).grid(row=0, column=0, sticky="w")
        ctk.CTkLabel(preview_header, text="(drag on the image to set a crop region)",
                     text_color=TEXT_DIM, font=ctk.CTkFont(size=11)
                     ).grid(row=0, column=1, sticky="w", padx=8)
        ctk.CTkButton(preview_header, text="Clear crop", width=90,
                      command=self.clear_crop,
                      fg_color="transparent", border_width=1,
                      border_color=TEXT_DIM, hover_color=LIST_HOVER
                      ).grid(row=0, column=2, sticky="e")

        self.canvas = tk.Canvas(right_card, bg=CANVAS_BG, highlightthickness=0, cursor="cross")
        self.canvas.grid(row=1, column=0, sticky="nsew", padx=12, pady=(0, 6))
        self.canvas.bind("<ButtonPress-1>", self.on_canvas_press)
        self.canvas.bind("<B1-Motion>", self.on_canvas_drag)
        self.canvas.bind("<ButtonRelease-1>", self.on_canvas_release)
        self.canvas.bind("<Configure>", lambda e: self._render_preview())

        self.crop_info = ctk.CTkLabel(right_card, text="No crop selected",
                                      text_color=TEXT_DIM,
                                      font=ctk.CTkFont(size=11), anchor="w")
        self.crop_info.grid(row=2, column=0, sticky="ew", padx=14, pady=(0, 10))

        settings = ctk.CTkFrame(self.root, fg_color=CARD_BG, corner_radius=10)
        settings.grid(row=2, column=0, sticky="ew", padx=16, pady=6)
        for c in range(8):
            settings.grid_columnconfigure(c, weight=0)
        settings.grid_columnconfigure(7, weight=1)

        ctk.CTkLabel(settings, text="Settings",
                     font=ctk.CTkFont(size=13, weight="bold")
                     ).grid(row=0, column=0, sticky="w", padx=14, pady=(12, 4), columnspan=8)

        ctk.CTkLabel(settings, text="Preset").grid(row=1, column=0, sticky="w", padx=(14, 4), pady=4)
        preset_row = ctk.CTkFrame(settings, fg_color="transparent")
        preset_row.grid(row=1, column=1, columnspan=7, sticky="w", pady=4)
        for label, w, h in PRESETS:
            ctk.CTkButton(
                preset_row, text=label, width=120, height=26,
                command=lambda w=w, h=h: self._apply_preset(w, h),
                fg_color="transparent", border_width=1,
                border_color=TEXT_DIM, hover_color=LIST_HOVER,
                font=ctk.CTkFont(size=11)
            ).pack(side="left", padx=(0, 4))

        ctk.CTkLabel(settings, text="Width").grid(row=2, column=0, sticky="w", padx=(14, 4), pady=4)
        ctk.CTkEntry(settings, textvariable=self.width_var, width=90, placeholder_text="px"
                     ).grid(row=2, column=1, sticky="w", pady=4)
        ctk.CTkLabel(settings, text="Height").grid(row=2, column=2, sticky="w", padx=(14, 4), pady=4)
        ctk.CTkEntry(settings, textvariable=self.height_var, width=90, placeholder_text="px"
                     ).grid(row=2, column=3, sticky="w", pady=4)
        ctk.CTkSwitch(settings, text="Keep aspect ratio",
                      variable=self.aspect_var, onvalue=True, offvalue=False
                      ).grid(row=2, column=4, sticky="w", padx=(20, 0), pady=4)
        ctk.CTkLabel(settings, text="leave blank to skip resize",
                     text_color=TEXT_DIM, font=ctk.CTkFont(size=11)
                     ).grid(row=2, column=5, sticky="w", padx=(20, 0), pady=4, columnspan=3)

        ctk.CTkLabel(settings, text="Quality").grid(row=3, column=0, sticky="w", padx=(14, 4), pady=4)
        self.quality_slider = ctk.CTkSlider(
            settings, from_=1, to=100, variable=self.quality_var,
            number_of_steps=99, width=320,
            command=lambda v: self.quality_label.configure(text=f"{int(float(v))}"))
        self.quality_slider.grid(row=3, column=1, columnspan=3, sticky="w", pady=4)
        self.quality_label = ctk.CTkLabel(
            settings, text=str(self.quality_var.get()), width=30,
            font=ctk.CTkFont(size=12, weight="bold"))
        self.quality_label.grid(row=3, column=4, sticky="w", padx=(8, 0))
        ctk.CTkLabel(settings, text="85-92 = best balance  -  100 on WebP = lossless",
                     text_color=TEXT_DIM, font=ctk.CTkFont(size=11)
                     ).grid(row=3, column=5, sticky="w", padx=(20, 0), pady=4, columnspan=3)

        ctk.CTkLabel(settings, text="Format").grid(row=4, column=0, sticky="w", padx=(14, 4), pady=4)
        ctk.CTkOptionMenu(settings, variable=self.format_var,
                          values=["Keep original", "JPG", "PNG", "WEBP"],
                          width=160, fg_color=LIST_BG, button_color=ACCENT,
                          button_hover_color=ACCENT_HOVER
                          ).grid(row=4, column=1, columnspan=2, sticky="w", pady=4)
        ctk.CTkLabel(settings, text="Suffix").grid(row=4, column=3, sticky="e", padx=(0, 4), pady=4)
        ctk.CTkEntry(settings, textvariable=self.suffix_var, width=180
                     ).grid(row=4, column=4, columnspan=2, sticky="w", pady=4)

        ctk.CTkLabel(settings, text="Output folder").grid(row=5, column=0, sticky="w", padx=(14, 4), pady=(4, 12))
        ctk.CTkEntry(settings, textvariable=self.output_var
                     ).grid(row=5, column=1, columnspan=6, sticky="ew", pady=(4, 12), padx=(0, 8))
        ctk.CTkButton(settings, text="Browse...", width=100,
                      command=self.browse_output,
                      fg_color="transparent", border_width=1,
                      border_color=TEXT_DIM, hover_color=LIST_HOVER
                      ).grid(row=5, column=7, sticky="e", padx=(0, 14), pady=(4, 12))

        action = ctk.CTkFrame(self.root, fg_color="transparent")
        action.grid(row=3, column=0, sticky="ew", padx=16, pady=(6, 6))
        action.grid_columnconfigure(0, weight=1)

        self.progress = ctk.CTkProgressBar(action, height=14, corner_radius=6,
                                           progress_color=ACCENT)
        self.progress.set(0)
        self.progress.grid(row=0, column=0, sticky="ew", padx=(0, 14))

        self.process_btn = ctk.CTkButton(
            action, text="Process selected (0)", width=210, height=38,
            command=self.start_processing,
            fg_color=SUCCESS, hover_color=SUCCESS_HOVER,
            font=ctk.CTkFont(size=13, weight="bold"))
        self.process_btn.grid(row=0, column=1, sticky="e")

        self.status = ctk.CTkLabel(self.root, text="Ready", anchor="w",
                                   text_color=TEXT_DIM, font=ctk.CTkFont(size=11))
        self.status.grid(row=4, column=0, sticky="ew", padx=18, pady=(0, 10))

    def _apply_preset(self, w, h):
        self.width_var.set(str(w) if w else "")
        self.height_var.set(str(h) if h else "")
        self.aspect_var.set(True)

    def _refresh_file_list(self):
        for w in self.file_list_frame.winfo_children():
            w.destroy()
        self.file_rows = []
        if not self.files:
            empty_hint = ('No files yet.\nDrag images or folders here,\nor click "Add files" / "Add folder".'
                          if DND_AVAILABLE else
                          'No files yet.\nClick "Add files" or "Add folder".')
            self.empty_label = ctk.CTkLabel(
                self.file_list_frame,
                text=empty_hint,
                text_color=TEXT_DIM, font=ctk.CTkFont(size=11),
                justify="center")
            self.empty_label.pack(pady=30)
            self._register_drop_targets()
            self._update_counts()
            return
        for i, p in enumerate(self.files):
            row = FileRow(
                self.file_list_frame, Path(p).name, i,
                on_click=self._select_index,
                on_check=self._on_row_check,
                checked=(p in self.checked_paths))
            row.pack(fill="x", pady=2)
            self.file_rows.append(row)
        if self.selected_index is not None and 0 <= self.selected_index < len(self.file_rows):
            self.file_rows[self.selected_index].set_selected(True)
        self._update_counts()

    def _update_counts(self):
        self.list_count.configure(text=f"{len(self.checked_paths)} of {len(self.files)} selected")
        self.process_btn.configure(text=f"Process selected ({len(self.checked_paths)})")

    def _on_row_check(self, index, checked):
        if index < 0 or index >= len(self.files):
            return
        path = self.files[index]
        if checked:
            self.checked_paths.add(path)
        else:
            self.checked_paths.discard(path)
        self._update_counts()

    def select_all(self):
        self.checked_paths = set(self.files)
        for row in self.file_rows:
            row.set_checked(True)
        self._update_counts()

    def select_none(self):
        self.checked_paths.clear()
        for row in self.file_rows:
            row.set_checked(False)
        self._update_counts()

    def _select_index(self, index):
        if index < 0 or index >= len(self.files):
            return
        for r in self.file_rows:
            r.set_selected(False)
        self.file_rows[index].set_selected(True)
        self.selected_index = index
        try:
            self.current_img = open_image(self.files[index])
        except Exception as e:
            messagebox.showerror("Cannot open image", str(e))
            self.current_img = None
            return
        self.clear_crop()
        self._render_preview()
        w, h = self.current_img.size
        self.status.configure(
            text=f"{Path(self.files[index]).name}   |   {w} x {h}px   |   "
                 f"{_human(os.path.getsize(self.files[index]))}")

    def add_files(self):
        paths = filedialog.askopenfilenames(
            title="Choose images",
            filetypes=[
                ("Images", "*.jpg *.jpeg *.png *.webp *.bmp *.tif *.tiff *.gif"),
                ("All files", "*.*")])
        self._add_paths(paths)

    def add_folder(self):
        folder = filedialog.askdirectory(title="Choose folder of images")
        if not folder:
            return
        paths = []
        for p in Path(folder).rglob("*"):
            if p.is_file() and p.suffix.lower() in SUPPORTED_EXTS:
                paths.append(str(p))
        self._add_paths(paths)

    def _add_paths(self, paths):
        added = 0
        for p in paths:
            if p not in self.files:
                self.files.append(p)
                self.checked_paths.add(p)
                added += 1
        self._refresh_file_list()
        self.status.configure(text=f"Added {added} file(s). Total: {len(self.files)}")
        if added and self.selected_index is None:
            self._select_index(0)

    def remove_current(self):
        if self.selected_index is None:
            return
        path = self.files[self.selected_index]
        del self.files[self.selected_index]
        self.checked_paths.discard(path)
        if not self.files:
            self.selected_index = None
            self.current_img = None
            self.canvas.delete("all")
        else:
            self.selected_index = min(self.selected_index, len(self.files) - 1)
        self._refresh_file_list()
        if self.selected_index is not None:
            self._select_index(self.selected_index)
        self.status.configure(text=f"Total: {len(self.files)}")

    def clear_files(self):
        self.files.clear()
        self.checked_paths.clear()
        self.selected_index = None
        self.current_img = None
        self.canvas.delete("all")
        self.clear_crop()
        self._refresh_file_list()
        self.status.configure(text="Cleared")

    def browse_output(self):
        folder = filedialog.askdirectory(
            title="Choose output folder",
            initialdir=self.output_var.get() or str(Path.home()))
        if folder:
            self.output_var.set(folder)

    def _render_preview(self):
        self.canvas.delete("all")
        if self.current_img is None:
            return
        cw = max(self.canvas.winfo_width(), 50)
        ch = max(self.canvas.winfo_height(), 50)
        iw, ih = self.current_img.size
        scale = min(cw / iw, ch / ih, 1.0)
        new_w = max(1, int(iw * scale))
        new_h = max(1, int(ih * scale))
        self.preview_scale = scale
        preview = self.current_img.copy()
        preview.thumbnail((new_w, new_h), Image.Resampling.LANCZOS)
        self.preview_img = preview
        self.preview_tk = ImageTk.PhotoImage(preview)
        ox = (cw - new_w) // 2
        oy = (ch - new_h) // 2
        self.preview_offset = (ox, oy)
        self.canvas.create_image(ox, oy, image=self.preview_tk, anchor="nw")
        if self.crop_box_preview:
            x1, y1, x2, y2 = self.crop_box_preview
            self.crop_rect_id = self.canvas.create_rectangle(
                x1, y1, x2, y2, outline=CROP_COLOR, width=2, dash=(4, 3))

    def _canvas_to_image(self, x, y):
        ox, oy = self.preview_offset
        ix = (x - ox) / max(self.preview_scale, 1e-9)
        iy = (y - oy) / max(self.preview_scale, 1e-9)
        return ix, iy

    def _clamp_to_preview(self, x, y):
        ox, oy = self.preview_offset
        pw, ph = self.preview_img.size if self.preview_img else (0, 0)
        x = max(ox, min(ox + pw, x))
        y = max(oy, min(oy + ph, y))
        return x, y

    def on_canvas_press(self, event):
        if self.current_img is None:
            return
        x, y = self._clamp_to_preview(event.x, event.y)
        self.crop_start = (x, y)
        if self.crop_rect_id:
            self.canvas.delete(self.crop_rect_id)
        self.crop_rect_id = self.canvas.create_rectangle(
            x, y, x, y, outline=CROP_COLOR, width=2, dash=(4, 3))

    def on_canvas_drag(self, event):
        if not self.crop_start or self.crop_rect_id is None:
            return
        x, y = self._clamp_to_preview(event.x, event.y)
        x1, y1 = self.crop_start
        self.canvas.coords(self.crop_rect_id, x1, y1, x, y)

    def on_canvas_release(self, event):
        if not self.crop_start or self.crop_rect_id is None:
            return
        x, y = self._clamp_to_preview(event.x, event.y)
        x1, y1 = self.crop_start
        x2, y2 = x, y
        if x2 < x1:
            x1, x2 = x2, x1
        if y2 < y1:
            y1, y2 = y2, y1
        if (x2 - x1) < 5 or (y2 - y1) < 5:
            self.canvas.delete(self.crop_rect_id)
            self.crop_rect_id = None
            self.crop_box_preview = None
            self.crop_info.configure(text="No crop selected")
            return
        self.crop_box_preview = (x1, y1, x2, y2)
        ix1, iy1 = self._canvas_to_image(x1, y1)
        ix2, iy2 = self._canvas_to_image(x2, y2)
        self.crop_info.configure(
            text=f"Crop region: {int(ix2 - ix1)} x {int(iy2 - iy1)}px  "
                 f"(from {int(ix1)},{int(iy1)})")

    def clear_crop(self):
        if self.crop_rect_id:
            self.canvas.delete(self.crop_rect_id)
        self.crop_rect_id = None
        self.crop_start = None
        self.crop_box_preview = None
        if hasattr(self, "crop_info"):
            self.crop_info.configure(text="No crop selected")

    def _current_crop_box_image(self):
        if not self.crop_box_preview:
            return None
        x1, y1, x2, y2 = self.crop_box_preview
        ix1, iy1 = self._canvas_to_image(x1, y1)
        ix2, iy2 = self._canvas_to_image(x2, y2)
        return (ix1, iy1, ix2, iy2)

    def start_processing(self):
        targets = [p for p in self.files if p in self.checked_paths]
        if not targets:
            messagebox.showinfo(
                "Nothing selected",
                "Tick the checkbox next to files you want to process, "
                "or click 'Select all'.")
            return
        out_dir = Path(self.output_var.get().strip() or "")
        if not str(out_dir):
            messagebox.showwarning("Output folder", "Please choose an output folder.")
            return
        try:
            out_dir.mkdir(parents=True, exist_ok=True)
        except Exception as e:
            messagebox.showerror("Cannot create output folder", str(e))
            return
        try:
            width = int(self.width_var.get()) if self.width_var.get().strip() else 0
            height = int(self.height_var.get()) if self.height_var.get().strip() else 0
            if width < 0 or height < 0:
                raise ValueError
        except ValueError:
            messagebox.showerror("Invalid size", "Width/height must be positive integers.")
            return
        quality = int(self.quality_var.get())
        keep_aspect = self.aspect_var.get()
        fmt_choice = self.format_var.get()
        suffix = self.suffix_var.get()
        crop_box = self._current_crop_box_image()

        self.process_btn.configure(state="disabled", text="Processing...")
        self.progress.set(0)

        t = threading.Thread(
            target=self._process_all,
            args=(targets, out_dir, width, height, keep_aspect, quality,
                  fmt_choice, suffix, crop_box),
            daemon=True)
        t.start()

    def _process_all(self, targets, out_dir, width, height, keep_aspect, quality,
                     fmt_choice, suffix, crop_box):
        ok = 0
        fail = 0
        total_in = 0
        total_out = 0
        total = len(targets)
        for i, src in enumerate(targets, start=1):
            try:
                size_in = os.path.getsize(src)
                total_in += size_in
                img = open_image(src)
                if crop_box:
                    img = crop_image(img, crop_box)
                if width or height:
                    img = resize_image(img, width, height, keep_aspect)
                src_path = Path(src)
                if fmt_choice == "Keep original":
                    out_ext = src_path.suffix.lower().lstrip(".")
                    if out_ext == "jpeg":
                        out_ext = "jpg"
                    fmt_to_use = "JPEG" if out_ext in ("jpg", "jpeg") else out_ext.upper()
                else:
                    out_ext = fmt_choice.lower()
                    fmt_to_use = fmt_choice
                out_name = f"{src_path.stem}{suffix}.{out_ext}"
                out_path = out_dir / out_name
                save_image(img, str(out_path), fmt_to_use, quality)
                size_out = os.path.getsize(out_path)
                total_out += size_out
                ok += 1
                self._set_status(
                    f"[{i}/{total}] {src_path.name}  ->  {out_name}   "
                    f"({_human(size_in)} -> {_human(size_out)})")
            except Exception as e:
                fail += 1
                self._set_status(f"Error on {Path(src).name}: {e}")
            self._set_progress(i / total)

        saved = total_in - total_out
        pct = (saved / total_in * 100) if total_in else 0
        summary = (f"Done. {ok} succeeded, {fail} failed.  "
                   f"Saved {_human(saved)} ({pct:.1f}%).")
        self._set_status(summary)
        self.root.after(0, lambda: self.process_btn.configure(
            state="normal",
            text=f"Process selected ({len(self.checked_paths)})"))
        self.root.after(0, lambda: messagebox.showinfo("Imaginify", summary))

    def _set_status(self, text):
        self.root.after(0, lambda: self.status.configure(text=text))

    def _set_progress(self, value):
        self.root.after(0, lambda: self.progress.set(value))


def main():
    root = CTkDnD()
    Imaginify(root)
    root.mainloop()


if __name__ == "__main__":
    main()
