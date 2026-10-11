# Writes the component catalog (one line per component) into the guide
# (crates/rustroke/src/lib.rs) and both READMEs. Run from the repo root
# after adding or changing a component: python3 tools/catalog.py
import re, sys
C = [
("Text", [
 ("Label", "Label", "Text, wrapping to the width; rich text with several styles and links (`Label::rich`). Shorthands: `ui.label`, `ui.heading`."),
 ("Hyperlink", "Hyperlink", "A link that opens a URL, or link-styled text that reacts to clicks."),
 ("Markdown", "Markdown", "A Markdown document: headings, lists, tables, code, images (feature `markdown`)."),
]),
("Buttons and choices", [
 ("Button", "Button", "A clickable button with text, an icon or both."),
 ("ToolButton", "ToolButton", "A large frameless toolbar icon, highlighted while its tool is active, optionally with a menu of variants."),
 ("IconToggle", "IconToggle", "An icon that switches a `bool` (eye open/closed, lock)."),
 ("Checkbox", "Checkbox", "A box that toggles a `bool`, with a label."),
 ("RadioButton", "RadioButton", "One choice of several; `ui.radio_value` sets a value when chosen."),
 ("SelectableLabel", "SelectableLabel", "Text highlighted when selected, for choices in lists and menus; `ui.selectable_value`."),
 ("ComboBox", "ComboBox", "A drop-down list showing the current choice."),
]),
("Values and input", [
 ("TextEdit", "TextEdit", "A field to edit text: one line, several lines, password, or code (monospace, Tab, line numbers, syntax colors)."),
 ("SearchField", "SearchField", "A one-line field for filtering, with a magnifying glass and a clear button."),
 ("Slider", "Slider", "A handle dragged along a track to pick a number in a range."),
 ("DragValue", "DragValue", "A number changed by dragging sideways or typed, with units and limits."),
 ("ColorPicker", "ColorPicker", "A color swatch that opens a picker (hue, saturation/value, opacity, hex)."),
 ("DatePicker", "DatePicker", "A date box that opens a calendar."),
 ("TimePicker", "TimePicker", "Hours and minutes, dragged or typed."),
 ("ReferenceField", "ReferenceField", "A reference to another object (a sketch, a face) with a button to clear it."),
]),
("Feedback", [
 ("ProgressBar", "ProgressBar", "A bar showing how much of a task is done, or that it is running."),
 ("Spinner", "Spinner", "A rotating arc: something is in progress."),
 ("Toast", "Toast", "A short message in a corner for a few seconds (`Context::toast`)."),
 ("Tooltip", "Response::on_hover_text", "Text shown while the pointer rests on a widget."),
]),
("Images and icons", [
 ("Image", "Image", "An image loaded with `Context::load_texture`, scaled and with rounded corners."),
 ("AnimatedImage", "AnimatedImage", "A GIF, APNG or animated WebP, played with the app's clock."),
 ("Icon", "Icon", "An SVG icon drawn in the theme's colors."),
]),
("Layout", [
 ("Stack", "Stack", "Widgets in a row, a column or one over the other, aligned, spread, wrapped and grown like flexbox, in a box with background, border, padding and size. `ui.horizontal` / `ui.vertical` are the short forms."),
 ("Grid", "Grid", "Widgets in rows and columns (content-sized, fixed or fractional), cells spanning several, in the same kind of box."),
 ("Separator", "Separator", "A thin line between groups; `ui.separator`."),
]),
("Areas and windows", [
 ("Panel", "Panel", "A bar along an edge of the window or of an area: toolbar, sidebar, status bar; resizable, with limits."),
 ("CentralPanel", "CentralPanel", "Everything left after the panels."),
 ("ScrollArea", "ScrollArea", "Content larger than its area, scrolled vertically, horizontally or both; only the visible rows of long lists."),
 ("Window", "Window", "A floating window inside the app: moved, resized, closed, brought to the front."),
 ("Modal", "Modal", "A dialog that blocks the rest of the window until it is answered."),
 ("CollapsingHeader", "CollapsingHeader", "A section that opens and closes; nested, a tree."),
 ("TabBar", "TabBar", "A row of tabs that can be closed, added and reordered."),
 ("DockArea", "DockArea", "Panels as tab groups in resizable splits; tabs are dragged between groups or to a side."),
]),
("Lists and data", [
 ("List", "List", "Rows of your items to select (also several) and reorder by dragging."),
 ("Tree", "Tree", "Nodes that expand and collapse, with selection, for any amount of data."),
 ("Table", "Table", "Rows and columns with a fixed header, resizable and sortable columns, millions of rows."),
 ("PropertyGrid", "PropertyGrid", "Names and editable values in two aligned columns, by sections (an inspector)."),
]),
("Charts", [
 ("Plot", "Plot", "Data on x/y axes: lines (solid, dashed, dotted), areas, points, bars (stacked, horizontal), histograms, heat maps, reference lines and text; ticks, grid, log scale, legend, tooltip, pan and zoom, live data with `PlotBuffer` and `Plot::follow_x`."),
 ("PieChart", "PieChart", "A pie or donut chart with percentages and a legend."),
]),
("Menus", [
 ("Menu", "Ui::menu_button", "A button that opens a menu, with submenus and icons; menu bars."),
 ("Context menu", "Response::context_menu", "A menu opened with a right click on a widget."),
 ("Popup", "Ui::popup_below", "Any content in a popup below a widget."),
]),
]

def guide():
    out = ["//! # Components", "//!",
           "//! Every component in one line, by category.", "//!"]
    for cat, rows in C:
        out += [f"//! **{cat}**", "//!", "//! | Component | What it is |", "//! |---|---|"]
        for name, link, desc in rows:
            label = f"[`{name}`]" if name == link else f"[{name}](`{link}`)"
            if name != link:
                label = f"[{name}]({link.replace('::', '::')})"
                label = f"{name} ([`{link}`])"
            out.append(f"//! | {label} | {desc} |")
        out.append("//!")
    return "\n".join(out) + "\n"

def readme():
    out = ["## Components", "", "Every component in one line, by category.", ""]
    for cat, rows in C:
        out += [f"**{cat}**", "", "| Component | What it is |", "|---|---|"]
        for name, link, desc in rows:
            label = f"`{name}`" if name == link else f"{name} (`{link}`)"
            out.append(f"| {label} | {desc} |")
        out.append("")
    return "\n".join(out) + "\n"

lib = 'crates/rustroke/src/lib.rs'
s = open(lib).read()
s = re.sub(r"//! # Components\n.*?(?=//! # Crates)", "", s, flags=re.S)
s = s.replace("//! # Crates", guide() + "//! # Crates", 1)
open(lib, 'w').write(s)
for p in ['README.md', 'crates/rustroke/README.md']:
    s = open(p).read()
    s = re.sub(r"## Components\n.*?(?=## Performance)", "", s, flags=re.S)
    s = s.replace("## Performance", readme() + "## Performance", 1)
    open(p, 'w').write(s)
