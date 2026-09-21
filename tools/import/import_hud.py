import argparse
import json
import re
from pathlib import Path

from import_camera_component import record
from ue_animation_asset_resolver import source_object_path
from ue_exported_text import parse_list, parse_quoted, parse_struct


def reference(value):
    return source_object_path(parse_quoted(value) if value.startswith('"') else value)


def dpi_curve(engine_ini, project_ini):
    settings = {}
    for text in (engine_ini, project_ini):
        section = ''
        for line in text.splitlines():
            line = line.strip()
            if line.startswith('['):
                section = line
            elif section == '[/Script/Engine.UserInterfaceSettings]' and '=' in line:
                key, value = line.split('=', 1)
                settings[key] = value
    if settings['UIScaleRule'] != 'ShortestSide':
        raise ValueError('Unsupported UI scale rule')
    curve = parse_struct(settings['UIScaleCurve'])
    if curve.get('ExternalCurve', 'None') != 'None':
        raise ValueError('External UI scale curve requires a native asset export')
    keys = parse_list(parse_struct(curve['EditorCurveData'])['Keys'])
    points = []
    for key in keys:
        fields = parse_struct(key)
        if fields.get('InterpMode', 'RCIM_Linear') != 'RCIM_Linear':
            raise ValueError('Unsupported UI scale interpolation')
        points.append((float(fields['Time']), float(fields['Value']) * float(settings.get('ApplicationScale', '1'))))
    if len(points) < 2 or any(a[0] >= b[0] for a, b in zip(points, points[1:])):
        raise ValueError('UI scale keys must be ordered')
    return points


def convert(source, engine_ini, project_ini, font_header):
    objects = {item['object_path']: item for item in source['asset_objects']}
    prefix = source['asset_path'] + ':WidgetTree'

    def prop(path, name):
        return record(objects[path]['properties'], name)['exported_text']

    def children(path):
        fields = {item['name']: item['exported_text'] for item in objects[path]['properties']}
        return [reference(prop(reference(slot), 'Content')) for slot in parse_list(fields.get('Slots') or '()')]

    render_dpi = int(re.search(r'RenderDPI\s*=\s*(\d+)', font_header)[1])
    texts = []
    rows = []
    row_gaps = set()

    def visit(path):
        cells = children(path)
        if objects[path]['class_path'] == '/Script/UMG.HorizontalBox' and all(objects[cell]['class_path'] == '/Script/UMG.Border' for cell in cells):
            row_gaps.add(float(parse_struct(prop(reference(prop(path, 'Slot')), 'Padding')).get('Top', '0')))
            row = []
            for cell in cells:
                slot = reference(prop(cell, 'Slot'))
                padding = parse_struct(prop(slot, 'Padding'))
                size = parse_struct(prop(slot, 'Size'))
                inset = parse_struct(prop(cell, 'Padding'))
                if size['SizeRule'] != 'Fill' or prop(slot, 'HorizontalAlignment') != 'HAlign_Fill':
                    raise ValueError('HUD column sizing changed')
                row.append((float(size['Value']), float(padding.get('Left', '0')), float(inset.get('Left', '0'))))
            rows.append(row)
        if objects[path]['class_path'] == '/Script/UMG.TextBlock':
            raw = prop(path, 'Text')
            text = parse_quoted(parse_list('(' + raw[len('NSLOCTEXT('):-1] + ')')[-1])
            font = parse_struct(prop(path, 'Font'))
            style = parse_quoted(font['TypefaceFontName'])
            if prop(path, 'AutoWrapText') != 'False' or prop(path, 'LineHeightPercentage') != '1.000000':
                raise ValueError('Text wrapping or line height changed')
            texts.append((text, float(font['Size']) * render_dpi / 72, 'Italic' in style, 'Bold' in style))
        for child in children(path):
            visit(child)

    root = reference(prop(prefix, 'RootWidget'))
    visit(root)
    size_box = children(root)[0]
    canvas = parse_struct(prop(reference(prop(size_box, 'Slot')), 'LayoutData'))
    offsets = parse_struct(canvas['Offsets'])
    if parse_struct(canvas['Alignment']) != {'X': '1.000000', 'Y': '0.000000'}:
        raise ValueError('HUD canvas alignment changed')
    constants = {
        'PANEL_WIDTH_PX': float(prop(size_box, 'WidthOverride')),
        'PANEL_RIGHT_MARGIN_PX': -float(offsets['Left']),
        'PANEL_TOP_MARGIN_PX': float(offsets['Top']),
        'FONT_SIZE_DEFAULT_PX': max(item[1] for item in texts),
        'FONT_SIZE_VALUE_PX': min(item[1] for item in texts),
    }
    section_gaps = {float(parse_struct(prop(reference(prop(section, 'Slot')), 'Padding')).get('Top', '0'))
                    for section in children(children(size_box)[0])[1:]}
    cell_gaps = {cell[1] for row in rows for cell in row[1:]}
    for name, values in [('ROW_GAP_PX', row_gaps), ('SECTION_GAP_PX', section_gaps), ('CELL_GAP_PX', cell_gaps)]:
        if len(values) != 1:
            raise ValueError(f'Nonuniform HUD spacing: {name}')
        constants[name] = values.pop()
    output = ''.join(f'pub const {name}: f32 = {value!r};\n' for name, value in constants.items())
    output += 'pub const UI_SCALE_CURVE: &[(f32, f32)] = &[\n'
    output += ''.join(f'    ({time!r}, {value!r}),\n' for time, value in dpi_curve(engine_ini, project_ini)) + '];\n'
    output += '#[cfg(test)]\nconst TEXT_STYLE_CONTRACT: &[(&str, f32, bool, bool)] = &[\n'
    output += ''.join(f'    ({json.dumps(text)}, {size!r}, {str(italic).lower()}, {str(bold).lower()}),\n' for text, size, italic, bold in texts) + '];\n'
    output += '#[cfg(test)]\nconst ROW_SLOT_CONTRACT: &[&[(f32, f32, f32)]] = &[\n'
    output += ''.join('    &[' + ', '.join(f'({weight!r}, {gap!r}, {padding!r})' for weight, gap, padding in row) + '],\n' for row in rows) + '];\n'
    return output


def self_test():
    ini = '[/Script/Engine.UserInterfaceSettings]\nUIScaleRule=ShortestSide\nUIScaleCurve=(EditorCurveData=(Keys=((Time=480,Value=0.444),(Time=1080,Value=1.0))))'
    assert dpi_curve(ini, '') == [(480, 0.444), (1080, 1.0)]
    assert dpi_curve(ini, '[/Script/Engine.UserInterfaceSettings]\nApplicationScale=2')[1] == (1080, 2)
    for invalid in (ini.replace('ShortestSide', 'LongestSide'), ini.replace('Time=1080', 'Time=480'), ini.replace('Time=480', 'InterpMode=RCIM_Cubic,Time=480')):
        try:
            dpi_curve(invalid, '')
        except ValueError:
            continue
        raise AssertionError('Invalid UI scale contract was accepted')


def convert_menu(menu, option, font_header):
    def widget(source, class_name):
        matches = [item for item in source['asset_objects'] if item['class_path'] == '/Script/UMG.' + class_name]
        if len(matches) != 1:
            raise ValueError(f'Expected one menu {class_name}')
        return {item['name']: item['exported_text'] for item in matches[0]['properties']}

    size = widget(option, 'SizeBox')
    border = widget(option, 'Border')
    text = widget(option, 'TextBlock')
    font = parse_struct(text['Font'])
    padding = parse_struct(border['Padding'])
    if (text['AutoWrapText'] != 'False' or text['Clipping'] != 'OnDemand' or text['TextOverflowPolicy'] != 'Ellipsis' or font['TypefaceFontName'] != '"Regular"'
            or padding['Left'] != padding['Right'] or size['bOverride_WidthOverride'] != 'True' or size['bOverride_HeightOverride'] != 'True'):
        raise ValueError('Menu text layout changed')
    graph = next(graph for graph in menu['graphs'] if graph['object_path'].endswith(':EventGraph'))
    setters = [node for node in graph['nodes'] if any(prop['name'] == 'FunctionReference' and
               parse_struct(prop['exported_text']).get('MemberName') == '"SetPadding"' for prop in node['properties'])]
    if len(setters) != 1:
        raise ValueError('Menu option spacing is ambiguous')
    gap = float(next(pin['default_value'] for pin in setters[0]['pins'] if pin['name'] == 'InPadding_Top'))
    render_dpi = int(re.search(r'RenderDPI\s*=\s*(\d+)', font_header)[1])
    constants = {'OPTION_WIDTH_PX': float(size['WidthOverride']), 'OPTION_HEIGHT_PX': float(size['HeightOverride']),
                 'OPTION_GAP_PX': gap, 'OPTION_FONT_SIZE_PX': float(font['Size']) * render_dpi / 72,
                 'OPTION_HORIZONTAL_PADDING_PX': float(padding['Left'])}
    output = ''.join(f'pub const {name}: f32 = {value!r};\n' for name, value in constants.items())
    selected_text = record(option['blueprint_cdo_properties'], 'TextSelectedColor')['exported_text']
    if selected_text != record(option['blueprint_cdo_properties'], 'TextNotSelectedColor')['exported_text']:
        raise ValueError('Menu text selection colors diverged')
    colors = {'COLOR_MENU_BACKGROUND': widget(menu, 'Border')['BrushColor'],
              'COLOR_MENU_TEXT': parse_struct(selected_text)['SpecifiedColor'],
              'COLOR_OPTION_SELECTED': record(option['blueprint_cdo_properties'], 'BorderSelectedColor')['exported_text'],
              'COLOR_OPTION_NOT_SELECTED': record(option['blueprint_cdo_properties'], 'BorderNotSelectedColor')['exported_text']}
    for name, raw in colors.items():
        color = parse_struct(raw)
        output += f'pub const {name}: Color = Color::linear_rgba(' + ', '.join(str(float(color[axis])) for axis in 'RGBA') + ');\n'
    return output


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source', type=Path)
    parser.add_argument('--menu', type=Path)
    parser.add_argument('--option', type=Path)
    parser.add_argument('--debug-fonts', type=Path)
    parser.add_argument('--engine', type=Path)
    parser.add_argument('--project', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        self_test()
    if args.source or args.menu or args.debug_fonts:
        font_header = (args.engine / 'Engine/Source/Runtime/SlateCore/Public/Fonts/SlateFontInfo.h').read_text(encoding='utf-8-sig')
        if args.debug_fonts:
            fonts = json.loads(args.debug_fonts.read_text(encoding='utf-8-sig'))['engine_debug_fonts']
            if fonts['tiny']['max_char_height'] <= 0 or fonts['tiny']['style'] != 'Light' or fonts['medium']['style'] != 'Regular':
                raise ValueError('Native debug font metrics are unavailable or changed')
            dpi = int(re.search(r'RenderDPI\s*=\s*(\d+)', font_header)[1])
            text = (f"const HOST_FONT_PX: f32 = {fonts['tiny']['size_points'] * dpi / 72!r};\n"
                    f"const HOST_MAX_CHAR_HEIGHT: f32 = {float(fonts['tiny']['max_char_height'])!r};\n"
                    f"const HEADER_FONT_PX: f32 = {fonts['medium']['size_points'] * dpi / 72!r};\n")
            output = args.output or Path('assets/ui/als_debug_canvas.runtime.rs')
        elif args.menu:
            text = convert_menu(json.loads(args.menu.read_text(encoding='utf-8-sig')),
                                json.loads(args.option.read_text(encoding='utf-8-sig')), font_header)
            output = args.output or Path('assets/ui/als_overlay_menu.runtime.rs')
        else:
            text = convert(json.loads(args.source.read_text(encoding='utf-8-sig')),
                (args.engine / 'Engine/Config/BaseEngine.ini').read_text(encoding='utf-8-sig'),
                (args.project / 'Config/DefaultEngine.ini').read_text(encoding='utf-8-sig'), font_header)
            output = args.output or Path('assets/ui/als_hud.runtime.rs')
        if args.check:
            if output.read_text(encoding='utf-8') != text:
                raise SystemExit('HUD data differs from native source')
        else:
            output.write_text(text, encoding='utf-8')
    print(json.dumps({'pass': True}))


if __name__ == '__main__':
    main()
