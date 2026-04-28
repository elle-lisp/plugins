//! Elle plotters plugin — data visualization via the plotters crate.
//!
//! Renders line charts, scatter plots, bar charts, histograms, and area charts
//! to PNG bytes or SVG strings.

use elle_plugin::{ElleResult, ElleValue, EllePrimDef, SIG_ERROR};

elle_plugin::define_plugin!("plotters/", &PRIMITIVES);

use plotters::coord::Shift;
use plotters::prelude::*;

// ── Types ────────────────────────────────────────────────────────────

struct ChartOpts {
    title: Option<String>,
    x_label: Option<String>,
    y_label: Option<String>,
    width: u32,
    height: u32,
    svg: bool,
    x_range: Option<(f64, f64)>,
    y_range: Option<(f64, f64)>,
    colors: Vec<RGBColor>,
    bins: usize,
    legend: bool,
}

impl Default for ChartOpts {
    fn default() -> Self {
        Self {
            title: None,
            x_label: None,
            y_label: None,
            width: 800,
            height: 600,
            svg: false,
            x_range: None,
            y_range: None,
            colors: default_palette(),
            bins: 20,
            legend: true,
        }
    }
}

#[derive(Clone, Copy)]
enum SeriesKind {
    Line,
    Scatter,
    Area,
    Bar,
}

struct SeriesSpec {
    kind: SeriesKind,
    label: Option<String>,
    data: Vec<(f64, f64)>,
    color: Option<RGBColor>,
    size: Option<u32>,
}

// ── Palette ──────────────────────────────────────────────────────────

fn default_palette() -> Vec<RGBColor> {
    vec![
        RGBColor(31, 119, 180),
        RGBColor(255, 127, 14),
        RGBColor(44, 160, 44),
        RGBColor(214, 39, 40),
        RGBColor(148, 103, 189),
        RGBColor(140, 86, 75),
        RGBColor(227, 119, 194),
        RGBColor(127, 127, 127),
        RGBColor(188, 189, 34),
        RGBColor(23, 190, 207),
    ]
}

// ── Parsing helpers ──────────────────────────────────────────────────

fn num(a: &elle_plugin::Api, v: ElleValue) -> Option<f64> {
    a.get_float(v).or_else(|| a.get_int(v).map(|i| i as f64))
}

fn parse_hex_color(s: &str) -> Option<RGBColor> {
    let s = s.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(RGBColor(r, g, b))
}

fn parse_color(a: &elle_plugin::Api, val: ElleValue) -> Option<RGBColor> {
    // Try hex string like "#ff0000"
    if let Some(s) = a.get_string(val) {
        return parse_hex_color(s);
    }
    // Try [r g b] array
    if let Some(len) = a.get_array_len(val) {
        if len >= 3 {
            let r = a.get_int(a.get_array_item(val, 0))? as u8;
            let g = a.get_int(a.get_array_item(val, 1))? as u8;
            let b = a.get_int(a.get_array_item(val, 2))? as u8;
            return Some(RGBColor(r, g, b));
        }
    }
    // Try keyword like :red
    match a.get_keyword_name(val)? {
        "red" => Some(RGBColor(214, 39, 40)),
        "blue" => Some(RGBColor(31, 119, 180)),
        "green" => Some(RGBColor(44, 160, 44)),
        "orange" => Some(RGBColor(255, 127, 14)),
        "purple" => Some(RGBColor(148, 103, 189)),
        "brown" => Some(RGBColor(140, 86, 75)),
        "pink" => Some(RGBColor(227, 119, 194)),
        "gray" | "grey" => Some(RGBColor(127, 127, 127)),
        "black" => Some(RGBColor(0, 0, 0)),
        "white" => Some(RGBColor(255, 255, 255)),
        "cyan" => Some(RGBColor(23, 190, 207)),
        "yellow" => Some(RGBColor(188, 189, 34)),
        _ => None,
    }
}

fn parse_range(a: &elle_plugin::Api, val: ElleValue) -> Option<(f64, f64)> {
    if a.get_array_len(val)? != 2 {
        return None;
    }
    Some((
        num(a, a.get_array_item(val, 0))?,
        num(a, a.get_array_item(val, 1))?,
    ))
}

fn parse_opts(a: &elle_plugin::Api, val: ElleValue) -> ChartOpts {
    let mut o = ChartOpts::default();
    if !a.check_struct(val) {
        return o;
    }
    if let Some(s) = a.get_string(a.get_struct_field(val, "title")) {
        o.title = Some(s.to_string());
    }
    if let Some(s) = a.get_string(a.get_struct_field(val, "x-label")) {
        o.x_label = Some(s.to_string());
    }
    if let Some(s) = a.get_string(a.get_struct_field(val, "y-label")) {
        o.y_label = Some(s.to_string());
    }
    if let Some(w) = a.get_int(a.get_struct_field(val, "width")) {
        o.width = w.max(1) as u32;
    }
    if let Some(h) = a.get_int(a.get_struct_field(val, "height")) {
        o.height = h.max(1) as u32;
    }
    if let Some(n) = a.get_int(a.get_struct_field(val, "bins")) {
        o.bins = n.max(1) as usize;
    }
    if a.get_keyword_name(a.get_struct_field(val, "format")) == Some("svg") {
        o.svg = true;
    }
    if let Some(r) = parse_range(a, a.get_struct_field(val, "x-range")) {
        o.x_range = Some(r);
    }
    if let Some(r) = parse_range(a, a.get_struct_field(val, "y-range")) {
        o.y_range = Some(r);
    }
    // :legend false — check for boolean false, int 0, or keyword :false
    let legend_val = a.get_struct_field(val, "legend");
    if let Some(b) = a.get_bool(legend_val) {
        o.legend = b;
    } else if a.get_int(legend_val) == Some(0) {
        o.legend = false;
    }
    if let Some(c) = parse_color(a, a.get_struct_field(val, "color")) {
        o.colors = vec![c];
    }
    let cv = a.get_struct_field(val, "colors");
    if let Some(len) = a.get_array_len(cv) {
        let cs: Vec<_> =
            (0..len).filter_map(|i| parse_color(a, a.get_array_item(cv, i))).collect();
        if !cs.is_empty() {
            o.colors = cs;
        }
    }
    o
}

fn extract_points(
    a: &elle_plugin::Api,
    data: ElleValue,
    name: &str,
) -> Result<Vec<(f64, f64)>, ElleResult> {
    let len = a.get_array_len(data).ok_or_else(|| {
        a.err(
            "type-error",
            &format!("{}: data must be array of [x y] pairs, got {}", name, a.type_name(data)),
        )
    })?;
    let mut pts = Vec::with_capacity(len);
    for i in 0..len {
        let p = a.get_array_item(data, i);
        let plen = a.get_array_len(p).ok_or_else(|| {
            a.err("type-error", &format!("{}: data[{}] must be [x y], got {}", name, i, a.type_name(p)))
        })?;
        if plen < 2 {
            return Err(a.err(
                "value-error",
                &format!("{}: data[{}] needs at least 2 elements", name, i),
            ));
        }
        let x = num(a, a.get_array_item(p, 0))
            .ok_or_else(|| a.err("type-error", &format!("{}: data[{}][0] not a number", name, i)))?;
        let y = num(a, a.get_array_item(p, 1))
            .ok_or_else(|| a.err("type-error", &format!("{}: data[{}][1] not a number", name, i)))?;
        pts.push((x, y));
    }
    Ok(pts)
}

fn extract_values(
    a: &elle_plugin::Api,
    data: ElleValue,
    name: &str,
) -> Result<Vec<f64>, ElleResult> {
    let len = a.get_array_len(data).ok_or_else(|| {
        a.err("type-error", &format!("{}: must be array of numbers, got {}", name, a.type_name(data)))
    })?;
    (0..len)
        .map(|i| {
            num(a, a.get_array_item(data, i))
                .ok_or_else(|| a.err("type-error", &format!("{}: [{}] not a number", name, i)))
        })
        .collect()
}

fn extract_labels(
    a: &elle_plugin::Api,
    data: ElleValue,
    name: &str,
) -> Result<Vec<String>, ElleResult> {
    let len = a.get_array_len(data).ok_or_else(|| {
        a.err("type-error", &format!("{}: labels must be array of strings, got {}", name, a.type_name(data)))
    })?;
    (0..len)
        .map(|i| {
            let v = a.get_array_item(data, i);
            a.get_string(v)
                .map(|s| s.to_string())
                .ok_or_else(|| a.err("type-error", &format!("{}: labels[{}] not a string", name, i)))
        })
        .collect()
}

// ── Auto-ranging ─────────────────────────────────────────────────────

fn auto_range(series: &[SeriesSpec]) -> ((f64, f64), (f64, f64)) {
    let (mut xlo, mut xhi) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut ylo, mut yhi) = (f64::INFINITY, f64::NEG_INFINITY);
    let has_bars = series.iter().any(|s| matches!(s.kind, SeriesKind::Bar));
    for s in series {
        for &(x, y) in &s.data {
            xlo = xlo.min(x);
            xhi = xhi.max(x);
            ylo = ylo.min(y);
            yhi = yhi.max(y);
        }
    }
    // Bar charts should always include y=0
    if has_bars {
        ylo = ylo.min(0.0);
        yhi = yhi.max(0.0);
    }
    if xlo == f64::INFINITY {
        return ((0.0, 1.0), (0.0, 1.0));
    }
    let xpad = (xhi - xlo).max(f64::EPSILON) * 0.05;
    let ypad = (yhi - ylo).max(f64::EPSILON) * 0.05;
    ((xlo - xpad, xhi + xpad), (ylo - ypad, yhi + ypad))
}

// ── PNG encoding ─────────────────────────────────────────────────────

fn rgb_to_png(buf: &[u8], w: u32, h: u32) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(std::io::Cursor::new(&mut out), w, h);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().map_err(|e| e.to_string())?;
        wr.write_image_data(buf).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

// ── Generic drawing ──────────────────────────────────────────────────

fn draw_xy<DB: DrawingBackend>(
    root: DrawingArea<DB, Shift>,
    series: &[SeriesSpec],
    opts: &ChartOpts,
) -> Result<(), String> {
    root.fill(&WHITE).map_err(|e| e.to_string())?;

    let (ax, ay) = auto_range(series);
    let xr = opts.x_range.unwrap_or(ax);
    let yr = opts.y_range.unwrap_or(ay);

    let mut b = ChartBuilder::on(&root);
    b.margin(20).x_label_area_size(40).y_label_area_size(50);
    if let Some(ref t) = opts.title {
        b.caption(t, ("sans-serif", 24));
    }

    let mut chart =
        b.build_cartesian_2d(xr.0..xr.1, yr.0..yr.1).map_err(|e| e.to_string())?;

    {
        let mut mesh = chart.configure_mesh();
        if let Some(ref l) = opts.x_label {
            mesh.x_desc(l);
        }
        if let Some(ref l) = opts.y_label {
            mesh.y_desc(l);
        }
        mesh.draw().map_err(|e| e.to_string())?;
    }

    let has_labels = series.iter().any(|s| s.label.is_some());
    for (i, s) in series.iter().enumerate() {
        let c = s.color.unwrap_or(opts.colors[i % opts.colors.len()]);
        match s.kind {
            SeriesKind::Line => {
                let w = s.size.unwrap_or(2);
                let d = chart
                    .draw_series(LineSeries::new(s.data.iter().copied(), c.stroke_width(w)))
                    .map_err(|e| e.to_string())?;
                if let Some(ref lbl) = s.label {
                    d.label(lbl).legend(move |(x, y)| {
                        PathElement::new(vec![(x, y), (x + 20, y)], c.stroke_width(w))
                    });
                }
            }
            SeriesKind::Scatter => {
                let r = s.size.unwrap_or(3);
                let d = chart
                    .draw_series(
                        s.data.iter().map(|&(x, y)| Circle::new((x, y), r, c.filled())),
                    )
                    .map_err(|e| e.to_string())?;
                if let Some(ref lbl) = s.label {
                    d.label(lbl)
                        .legend(move |(x, y)| Circle::new((x, y), r, c.filled()));
                }
            }
            SeriesKind::Area => {
                let d = chart
                    .draw_series(
                        AreaSeries::new(s.data.iter().copied(), 0.0, c.mix(0.3))
                            .border_style(c.stroke_width(2)),
                    )
                    .map_err(|e| e.to_string())?;
                if let Some(ref lbl) = s.label {
                    d.label(lbl).legend(move |(x, y)| {
                        Rectangle::new([(x, y - 5), (x + 20, y + 5)], c.mix(0.3).filled())
                    });
                }
            }
            SeriesKind::Bar => {
                // Collect all x positions across all bar series to determine layout.
                let mut all_bar_xs: Vec<f64> = series.iter()
                    .filter(|ss| matches!(ss.kind, SeriesKind::Bar))
                    .flat_map(|ss| ss.data.iter().map(|&(x, _)| x))
                    .collect();
                all_bar_xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
                all_bar_xs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

                // Count how many series share each x position
                let max_at_same_x = {
                    let mut max = 0usize;
                    for &xv in &all_bar_xs {
                        let count = series.iter()
                            .filter(|ss| matches!(ss.kind, SeriesKind::Bar))
                            .filter(|ss| ss.data.iter().any(|&(x, _)| (x - xv).abs() < 1e-9))
                            .count();
                        max = max.max(count);
                    }
                    max.max(1)
                };

                // Min gap between distinct x positions
                let min_gap = if all_bar_xs.len() >= 2 {
                    all_bar_xs.windows(2)
                        .map(|w| w[1] - w[0])
                        .fold(f64::INFINITY, f64::min)
                } else {
                    (xr.1 - xr.0).max(1.0)
                };

                let total_bar_width = min_gap * 0.8;
                let single_width = total_bar_width / max_at_same_x as f64;

                // This series' index among those sharing each x
                let bar_idx = series[..i].iter()
                    .filter(|ss| matches!(ss.kind, SeriesKind::Bar))
                    .count() % max_at_same_x;
                let group_offset = -total_bar_width / 2.0 + single_width * bar_idx as f64;

                let d = chart
                    .draw_series(s.data.iter().map(|&(x, y)| {
                        let x0 = x + group_offset;
                        Rectangle::new([(x0, 0.0), (x0 + single_width, y)], c.filled())
                    }))
                    .map_err(|e| e.to_string())?;
                if let Some(ref lbl) = s.label {
                    d.label(lbl).legend(move |(x, y)| {
                        Rectangle::new([(x, y - 5), (x + 20, y + 5)], c.filled())
                    });
                }
            }
        }
    }

    if has_labels && opts.legend {
        chart
            .configure_series_labels()
            .background_style(WHITE.mix(0.8))
            .border_style(BLACK)
            .draw()
            .map_err(|e| e.to_string())?;
    }

    root.present().map_err(|e| e.to_string())?;
    Ok(())
}

fn draw_bars<DB: DrawingBackend>(
    root: DrawingArea<DB, Shift>,
    labels: &[String],
    values: &[f64],
    opts: &ChartOpts,
) -> Result<(), String> {
    root.fill(&WHITE).map_err(|e| e.to_string())?;

    let max_y = values.iter().copied().fold(0.0f64, f64::max);
    let yr = (
        opts.y_range.map(|r| r.0).unwrap_or(0.0),
        opts.y_range
            .map(|r| r.1)
            .unwrap_or(if max_y < f64::EPSILON { 1.0 } else { max_y * 1.1 }),
    );

    let mut b = ChartBuilder::on(&root);
    b.margin(20).x_label_area_size(40).y_label_area_size(50);
    if let Some(ref t) = opts.title {
        b.caption(t, ("sans-serif", 24));
    }

    let n = labels.len() as i32;
    let mut chart =
        b.build_cartesian_2d((0..n).into_segmented(), yr.0..yr.1)
            .map_err(|e| e.to_string())?;

    let lab = labels.to_vec();
    let formatter = move |v: &SegmentValue<i32>| match v {
        SegmentValue::CenterOf(i) => lab.get(*i as usize).cloned().unwrap_or_default(),
        _ => String::new(),
    };
    {
        let mut mesh = chart.configure_mesh();
        mesh.x_label_formatter(&formatter);
        if let Some(ref l) = opts.x_label {
            mesh.x_desc(l);
        }
        if let Some(ref l) = opts.y_label {
            mesh.y_desc(l);
        }
        mesh.draw().map_err(|e| e.to_string())?;
    }

    let c = opts.colors[0];
    chart
        .draw_series((0i32..).zip(values.iter()).map(|(i, &v)| {
            Rectangle::new(
                [(SegmentValue::Exact(i), 0.0), (SegmentValue::Exact(i + 1), v)],
                c.filled(),
            )
        }))
        .map_err(|e| e.to_string())?;

    root.present().map_err(|e| e.to_string())?;
    Ok(())
}

fn draw_hist<DB: DrawingBackend>(
    root: DrawingArea<DB, Shift>,
    values: &[f64],
    opts: &ChartOpts,
) -> Result<(), String> {
    root.fill(&WHITE).map_err(|e| e.to_string())?;
    if values.is_empty() {
        root.present().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let vmin = values.iter().copied().fold(f64::INFINITY, f64::min);
    let vmax = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let nb = opts.bins;
    let (lo, bw) = if (vmax - vmin).abs() < f64::EPSILON {
        (vmin - 0.5, 1.0 / nb as f64)
    } else {
        (vmin, (vmax - vmin) / nb as f64)
    };

    let mut bins = vec![0u32; nb];
    for &v in values {
        let i = ((v - lo) / bw).floor() as usize;
        bins[i.min(nb - 1)] += 1;
    }
    let max_count = *bins.iter().max().unwrap_or(&1);

    let xr = opts.x_range.unwrap_or((lo, lo + bw * nb as f64));
    let yr = (
        0.0,
        opts.y_range
            .map(|r| r.1)
            .unwrap_or(max_count as f64 * 1.1),
    );

    let mut b = ChartBuilder::on(&root);
    b.margin(20).x_label_area_size(40).y_label_area_size(50);
    if let Some(ref t) = opts.title {
        b.caption(t, ("sans-serif", 24));
    }

    let mut chart =
        b.build_cartesian_2d(xr.0..xr.1, yr.0..yr.1).map_err(|e| e.to_string())?;

    {
        let mut mesh = chart.configure_mesh();
        if let Some(ref l) = opts.x_label {
            mesh.x_desc(l);
        }
        if let Some(ref l) = opts.y_label {
            mesh.y_desc(l);
        }
        mesh.draw().map_err(|e| e.to_string())?;
    }

    let c = opts.colors[0];
    chart
        .draw_series(bins.iter().enumerate().map(|(i, &count)| {
            let x0 = lo + i as f64 * bw;
            Rectangle::new([(x0, 0.0), (x0 + bw, count as f64)], c.filled())
        }))
        .map_err(|e| e.to_string())?;

    root.present().map_err(|e| e.to_string())?;
    Ok(())
}

// ── Backend dispatch ─────────────────────────────────────────────────

fn render_xy_chart(series: &[SeriesSpec], opts: &ChartOpts) -> Result<ElleValue, String> {
    let a = api();
    if opts.svg {
        let mut buf = String::new();
        {
            let root = SVGBackend::with_string(&mut buf, (opts.width, opts.height))
                .into_drawing_area();
            draw_xy(root, series, opts)?;
        }
        Ok(a.string(&buf))
    } else {
        let mut px = vec![0u8; (opts.width as usize) * (opts.height as usize) * 3];
        {
            let root =
                BitMapBackend::with_buffer(&mut px, (opts.width, opts.height)).into_drawing_area();
            draw_xy(root, series, opts)?;
        }
        Ok(a.bytes(&rgb_to_png(&px, opts.width, opts.height)?))
    }
}

fn render_bar_chart(
    labels: &[String],
    values: &[f64],
    opts: &ChartOpts,
) -> Result<ElleValue, String> {
    let a = api();
    if opts.svg {
        let mut buf = String::new();
        {
            let root = SVGBackend::with_string(&mut buf, (opts.width, opts.height))
                .into_drawing_area();
            draw_bars(root, labels, values, opts)?;
        }
        Ok(a.string(&buf))
    } else {
        let mut px = vec![0u8; (opts.width as usize) * (opts.height as usize) * 3];
        {
            let root =
                BitMapBackend::with_buffer(&mut px, (opts.width, opts.height)).into_drawing_area();
            draw_bars(root, labels, values, opts)?;
        }
        Ok(a.bytes(&rgb_to_png(&px, opts.width, opts.height)?))
    }
}

fn render_histogram(values: &[f64], opts: &ChartOpts) -> Result<ElleValue, String> {
    let a = api();
    if opts.svg {
        let mut buf = String::new();
        {
            let root = SVGBackend::with_string(&mut buf, (opts.width, opts.height))
                .into_drawing_area();
            draw_hist(root, values, opts)?;
        }
        Ok(a.string(&buf))
    } else {
        let mut px = vec![0u8; (opts.width as usize) * (opts.height as usize) * 3];
        {
            let root =
                BitMapBackend::with_buffer(&mut px, (opts.width, opts.height)).into_drawing_area();
            draw_hist(root, values, opts)?;
        }
        Ok(a.bytes(&rgb_to_png(&px, opts.width, opts.height)?))
    }
}

// ── Primitives ───────────────────────────────────────────────────────

extern "C" fn prim_line(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let pts = match extract_points(a, unsafe { a.arg(args, nargs, 0) }, "plotters/line") {
        Ok(p) => p,
        Err(e) => return e,
    };
    let opts =
        if nargs > 1 { parse_opts(a, unsafe { a.arg(args, nargs, 1) }) } else { ChartOpts::default() };
    let series = vec![SeriesSpec { kind: SeriesKind::Line, label: None, data: pts, color: None, size: None }];
    match render_xy_chart(&series, &opts) {
        Ok(v) => a.ok(v),
        Err(e) => a.err("plotters-error", &format!("plotters/line: {}", e)),
    }
}

extern "C" fn prim_scatter(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let pts = match extract_points(a, unsafe { a.arg(args, nargs, 0) }, "plotters/scatter") {
        Ok(p) => p,
        Err(e) => return e,
    };
    let opts =
        if nargs > 1 { parse_opts(a, unsafe { a.arg(args, nargs, 1) }) } else { ChartOpts::default() };
    let series =
        vec![SeriesSpec { kind: SeriesKind::Scatter, label: None, data: pts, color: None, size: None }];
    match render_xy_chart(&series, &opts) {
        Ok(v) => a.ok(v),
        Err(e) => a.err("plotters-error", &format!("plotters/scatter: {}", e)),
    }
}

extern "C" fn prim_area(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let pts = match extract_points(a, unsafe { a.arg(args, nargs, 0) }, "plotters/area") {
        Ok(p) => p,
        Err(e) => return e,
    };
    let opts =
        if nargs > 1 { parse_opts(a, unsafe { a.arg(args, nargs, 1) }) } else { ChartOpts::default() };
    let series = vec![SeriesSpec { kind: SeriesKind::Area, label: None, data: pts, color: None, size: None }];
    match render_xy_chart(&series, &opts) {
        Ok(v) => a.ok(v),
        Err(e) => a.err("plotters-error", &format!("plotters/area: {}", e)),
    }
}

extern "C" fn prim_bar(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let labels = match extract_labels(a, unsafe { a.arg(args, nargs, 0) }, "plotters/bar") {
        Ok(l) => l,
        Err(e) => return e,
    };
    let values = match extract_values(a, unsafe { a.arg(args, nargs, 1) }, "plotters/bar") {
        Ok(v) => v,
        Err(e) => return e,
    };
    if labels.len() != values.len() {
        return a.err(
            "value-error",
            &format!(
                "plotters/bar: labels ({}) and values ({}) length mismatch",
                labels.len(),
                values.len()
            ),
        );
    }
    let opts =
        if nargs > 2 { parse_opts(a, unsafe { a.arg(args, nargs, 2) }) } else { ChartOpts::default() };
    match render_bar_chart(&labels, &values, &opts) {
        Ok(v) => a.ok(v),
        Err(e) => a.err("plotters-error", &format!("plotters/bar: {}", e)),
    }
}

extern "C" fn prim_histogram(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let vals = match extract_values(a, unsafe { a.arg(args, nargs, 0) }, "plotters/histogram") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let opts = if nargs > 1 {
        parse_opts(a, unsafe { a.arg(args, nargs, 1) })
    } else {
        ChartOpts::default()
    };
    match render_histogram(&vals, &opts) {
        Ok(v) => a.ok(v),
        Err(e) => a.err("plotters-error", &format!("plotters/histogram: {}", e)),
    }
}

extern "C" fn prim_chart(args: *const ElleValue, nargs: usize) -> ElleResult {
    let a = api();
    let spec = unsafe { a.arg(args, nargs, 0) };
    if !a.check_struct(spec) {
        return a.err(
            "type-error",
            &format!("plotters/chart: expected struct, got {}", a.type_name(spec)),
        );
    }
    let opts = parse_opts(a, spec);
    let sv = a.get_struct_field(spec, "series");
    let slen = match a.get_array_len(sv) {
        Some(n) => n,
        None => return a.err("type-error", "plotters/chart: :series must be an array"),
    };
    let mut series = Vec::with_capacity(slen);
    for i in 0..slen {
        let s = a.get_array_item(sv, i);
        if !a.check_struct(s) {
            return a.err(
                "type-error",
                &format!("plotters/chart: series[{}] must be a struct", i),
            );
        }
        let kind = match a.get_keyword_name(a.get_struct_field(s, "type")) {
            Some("line") => SeriesKind::Line,
            Some("scatter") => SeriesKind::Scatter,
            Some("area") => SeriesKind::Area,
            Some("bar") => SeriesKind::Bar,
            _ => {
                return a.err(
                    "value-error",
                    &format!(
                        "plotters/chart: series[{}] :type must be :line, :scatter, :area, or :bar",
                        i
                    ),
                )
            }
        };
        let label = a.get_string(a.get_struct_field(s, "label")).map(|s| s.to_string());
        let data = match extract_points(
            a,
            a.get_struct_field(s, "data"),
            &format!("plotters/chart series[{}]", i),
        ) {
            Ok(d) => d,
            Err(e) => return e,
        };
        let color = parse_color(a, a.get_struct_field(s, "color"));
        let size = a.get_int(a.get_struct_field(s, "size")).map(|v| v.max(1) as u32);
        series.push(SeriesSpec { kind, label, data, color, size });
    }
    match render_xy_chart(&series, &opts) {
        Ok(v) => a.ok(v),
        Err(e) => a.err("plotters-error", &format!("plotters/chart: {}", e)),
    }
}

// ── Registration ─────────────────────────────────────────────────────

static PRIMITIVES: &[EllePrimDef] = &[
    EllePrimDef::range(
        "plotters/line",
        prim_line,
        SIG_ERROR,
        1,
        2,
        "Line chart from [[x y] ...] data. Returns PNG bytes or SVG string (:format :svg in opts).",
        "plotters",
        "(plotters/line [[1 20] [2 22] [3 19] [4 25]])",
    ),
    EllePrimDef::range(
        "plotters/scatter",
        prim_scatter,
        SIG_ERROR,
        1,
        2,
        "Scatter plot from [[x y] ...] data.",
        "plotters",
        "(plotters/scatter [[1 20] [2 22] [3 19] [4 25]])",
    ),
    EllePrimDef::range(
        "plotters/area",
        prim_area,
        SIG_ERROR,
        1,
        2,
        "Area chart from [[x y] ...] data.",
        "plotters",
        "(plotters/area [[1 20] [2 22] [3 19] [4 25]])",
    ),
    EllePrimDef::range(
        "plotters/bar",
        prim_bar,
        SIG_ERROR,
        2,
        3,
        "Bar chart from labels array and values array.",
        "plotters",
        "(plotters/bar [\"Mon\" \"Tue\" \"Wed\"] [10 20 15])",
    ),
    EllePrimDef::range(
        "plotters/histogram",
        prim_histogram,
        SIG_ERROR,
        1,
        2,
        "Histogram with auto-binning. :bins in opts sets bin count (default 20).",
        "plotters",
        "(plotters/histogram [1.2 3.4 2.1 4.5 3.3 2.8])",
    ),
    EllePrimDef::exact(
        "plotters/chart",
        prim_chart,
        SIG_ERROR,
        1,
        "Multi-series chart. Spec: {:series [{:type :line/:scatter/:area/:bar :data [[x y]...] :label str :color}] :title :x-label :y-label :width :height :format :x-range :y-range}. Colors accept :keyword, [r g b], or \"#rrggbb\".",
        "plotters",
        "(plotters/chart {:title \"Compare\" :series [{:type :line :label \"A\" :data [[1 10] [2 20]]}]})",
    ),
];
