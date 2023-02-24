use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::take;
use nom::combinator::{complete, map, map_parser};
use nom::multi::{count, many0};
use nom::number::complete::{be_f64, be_i16, be_i24, be_i32, be_i64, be_i8, be_u16, be_u32, be_u8};
use nom::sequence::{pair, Tuple};
use nom::IResult;

use crate::model::*;
use crate::varint::be_u64_varint;

pub fn database(i: &[u8]) -> IResult<&[u8], Database> {
    let (i, header) = db_header(i)?;

    const HEADER_SIZE: usize = 100;
    let page_size = header.page_size.real_size();

    // root page is smaller than the rest
    let (i, root_page) = map_parser(take(page_size - HEADER_SIZE), page::<HEADER_SIZE>)(i)?;

    let (i, mut pages) = complete(many0(map_parser(take(page_size), page::<0>)))(i)?;
    pages.insert(0, root_page);

    Ok((i, Database { header, pages }))
}

fn db_header(i: &[u8]) -> IResult<&[u8], DbHeader> {
    let (i, _) = tag("SQLite format 3\0")(i)?;
    let (i, page_size) = map(be_u16, |p| PageSize(p))(i)?;
    let (i, (write_version, read_version)) = (be_u8, be_u8).parse(i)?;
    let (i, _reserved) = be_u8(i)?;
    let (i, (max_payload_fraction, min_payload_fraction, leaf_payload_fraction)) =
        (be_u8, be_u8, be_u8).parse(i)?;
    let (i, file_change_counter) = be_u32(i)?;
    let (i, db_size) = be_u32(i)?;
    let (i, (first_freelist_page_no, total_freelist_pages)) = (be_u32, be_u32).parse(i)?;
    let (i, (schema_cookie, schema_format_no)) = (be_u32, be_u32).parse(i)?;
    let (i, default_page_cache_size) = be_u32(i)?;
    let (i, no_largest_root_b_tree) = be_u32(i)?;
    let (i, db_text_encoding) = be_u32(i)?;
    let (i, user_version) = be_u32(i)?;
    let (i, incremental_vacuum_mode) = be_u32(i)?;
    let (i, application_id) = be_u32(i)?;
    let (i, _reserved) = count(be_u8, 20)(i)?;
    let (i, (version_valid_for_no, sqlite_version_number)) = (be_u32, be_u32).parse(i)?;

    Ok((
        i,
        DbHeader {
            page_size,
            write_version,
            read_version,
            max_payload_fraction,
            min_payload_fraction,
            leaf_payload_fraction,
            file_change_counter,
            db_size,
            first_freelist_page_no,
            total_freelist_pages,
            schema_cookie,
            schema_format_no,
            default_page_cache_size,
            no_largest_root_b_tree,
            db_text_encoding,
            user_version,
            incremental_vacuum_mode,
            application_id,
            version_valid_for_no,
            sqlite_version_number,
        },
    ))
}

// todo: fix const generic thing, hack to pass through parameters
fn page<const OFFSET: usize>(i: &[u8]) -> IResult<&[u8], Page> {
    alt((
        map(interior_index_b_tree_page::<OFFSET>, |p| {
            Page::InteriorIndexPage(p)
        }),
        map(leaf_index_b_tree_page::<OFFSET>, |p| Page::LeafIndexPage(p)),
        map(interior_table_b_tree_page::<OFFSET>, |p| {
            Page::InteriorTablePage(p)
        }),
        map(leaf_table_b_tree_page::<OFFSET>, |p| Page::LeafTablePage(p)),
    ))(i)
}

fn interior_page_header(i: &[u8]) -> IResult<&[u8], InteriorPageHeader> {
    let (i, first_freeblock_offset) = map(be_u16, |u| Some(u).filter(|&p| p != 0x0u16))(i)?;
    let (i, no_cells) = be_u16(i)?;
    let (i, cell_content_offset) = map(be_u16, |u| CellOffset(u))(i)?;
    let (i, no_fragmented_bytes) = be_u8(i)?;
    let (i, rightmost_pointer) = be_u32(i)?;

    Ok((
        i,
        InteriorPageHeader {
            first_freeblock_offset,
            no_cells,
            cell_content_offset,
            no_fragmented_bytes,
            rightmost_pointer,
        },
    ))
}

fn leaf_page_header(i: &[u8]) -> IResult<&[u8], LeafPageHeader> {
    let (i, first_freeblock_offset) = map(be_u16, |u| Some(u).filter(|&p| p != 0x0u16))(i)?;
    let (i, no_cells) = be_u16(i)?;
    let (i, cell_content_offset) = map(be_u16, |u| CellOffset(u))(i)?;
    let (i, no_fragmented_bytes) = be_u8(i)?;

    Ok((
        i,
        LeafPageHeader {
            first_freeblock_offset,
            no_cells,
            cell_content_offset,
            no_fragmented_bytes,
        },
    ))
}

fn interior_index_b_tree_page<const OFFSET: usize>(i: &[u8]) -> IResult<&[u8], InteriorIndexPage> {
    let (ii, _) = tag([0x02u8])(i)?;
    let (ii, header) = interior_page_header(ii)?;
    let (ii, cell_pointers) = count(be_u16, header.no_cells.into())(ii)?;

    let mut cells = Vec::with_capacity(cell_pointers.len());
    for &ptr in cell_pointers.iter() {
        let (_, cell) = interior_index_cell(&i[(ptr as usize - OFFSET)..])?;
        cells.push(cell);
    }

    Ok((
        ii,
        InteriorIndexPage {
            header,
            cell_pointers,
            cells,
        },
    ))
}

/// Expects to get exactly as many bytes in input as it will consume
fn column_types(i: &[u8]) -> IResult<&[u8], Vec<SerialType>> {
    // many0 as header might actually be empty
    complete(many0(map(be_u64_varint, SerialType::from)))(i)
}

fn be_i48(i: &[u8]) -> IResult<&[u8], i64> {
    let (i, (head, tail)) = pair(be_u16, be_u32)(i)?;
    let mut x = (head as u64) << 32 | (tail as u64);
    if x & 0x80_00_00_00_00_00 != 0 {
        x |= 0xff_ff_00_00_00_00_00_00;
    };

    Ok((i, x as i64))
}

fn column_values<'a, 'b>(
    i: &'a [u8],
    serial_types: &'b [SerialType],
) -> IResult<&'a [u8], Vec<Option<Payload>>> {
    let mut i: &[u8] = i;
    let mut res = Vec::with_capacity(serial_types.len());
    for serial_type in serial_types {
        let (ii, v) = match serial_type {
            SerialType::Null => Ok((i, None)),
            SerialType::I8 => map(be_i8, |x| Some(Payload::I8(x)))(i),
            SerialType::I16 => map(be_i16, |x| Some(Payload::I16(x)))(i),
            SerialType::I24 => map(be_i24, |x| Some(Payload::I32(x)))(i),
            SerialType::I32 => map(be_i32, |x| Some(Payload::I32(x)))(i),
            SerialType::I48 => map(be_i48, |x| Some(Payload::I64(x)))(i),
            SerialType::I64 => map(be_i64, |x| Some(Payload::I64(x)))(i),
            SerialType::F64 => map(be_f64, |x| Some(Payload::F64(x)))(i),
            SerialType::Const0 => Ok((i, Some(Payload::I8(0)))),
            SerialType::Const1 => Ok((i, Some(Payload::I8(0)))),
            SerialType::Reserved => unimplemented!("reserved"),
            SerialType::Blob(_) if serial_type.size() == 0 => Ok((i, None)),
            SerialType::Blob(_) => map(take(serial_type.size()), |x: &[u8]| {
                Some(Payload::Blob(x.to_vec()))
            })(i),
            SerialType::Text(_) if serial_type.size() == 0 => Ok((i, None)),
            // todo: parse string encodings
            SerialType::Text(_) => map(take(serial_type.size()), |x: &[u8]| {
                let x = String::from_utf8(x.to_vec()).unwrap();
                Some(Payload::Text(x))
            })(i),
        }?;
        i = ii;
        dbg!(v.clone());
        res.push(v);
    }

    Ok((i, res))
}

fn index_cell_payload(i: &[u8]) -> IResult<&[u8], IndexCellPayload> {
    let (i, header_size) = be_u64_varint(i)?;
    let (_, column_types) = column_types(&i[0..header_size as usize - 1])?;
    let (i, column_values) = column_values(&i[header_size as usize - 1..], &column_types)?;
    let (i, rowid) = be_u64_varint(i)?;

    Ok((
        i,
        IndexCellPayload {
            header_size,
            column_types,
            column_values,
            rowid,
        },
    ))
}

fn interior_index_cell(i: &[u8]) -> IResult<&[u8], InteriorIndexCell> {
    let (i, left_child_page_no) = be_u32(i)?;
    let (i, payload_size) = be_u64_varint(i)?;
    let (i, payload) = index_cell_payload(i)?;

    Ok((
        i,
        InteriorIndexCell {
            left_child_page_no,
            payload_size,
            payload,
            overflow_page_no: None,
        },
    ))
}

fn interior_table_b_tree_page<const OFFSET: usize>(i: &[u8]) -> IResult<&[u8], InteriorTablePage> {
    let (ii, _) = tag([0x05u8])(i)?;
    let (ii, header) = interior_page_header(ii)?;
    let (ii, cell_pointers) = count(be_u16, header.no_cells.into())(ii)?;

    let mut cells = Vec::with_capacity(cell_pointers.len());
    for &ptr in cell_pointers.iter() {
        let (_, cell) = interior_table_cell(&i[(ptr as usize - OFFSET)..])?;
        cells.push(cell);
    }

    Ok((
        ii,
        InteriorTablePage {
            header,
            cell_pointers,
            cells,
        },
    ))
}

fn interior_table_cell(i: &[u8]) -> IResult<&[u8], InteriorTableCell> {
    let (i, left_child_page_no) = be_u32(i)?;
    let (i, integer_key) = be_u64_varint(i)?;

    Ok((
        i,
        InteriorTableCell {
            left_child_page_no,
            integer_key,
        },
    ))
}

fn leaf_index_b_tree_page<const OFFSET: usize>(i: &[u8]) -> IResult<&[u8], LeafIndexPage> {
    let (ii, _) = tag([0x0au8])(i)?;
    let (ii, header) = leaf_page_header(ii)?;
    let (ii, cell_pointers) = count(be_u16, header.no_cells.into())(ii)?;

    let mut cells = Vec::with_capacity(cell_pointers.len());
    for &ptr in cell_pointers.iter() {
        let (_, cell) = leaf_index_cell(&i[(ptr as usize - OFFSET)..])?;
        cells.push(cell);
    }

    Ok((
        ii,
        LeafIndexPage {
            header,
            cell_pointers,
            cells,
        },
    ))
}

fn leaf_index_cell(i: &[u8]) -> IResult<&[u8], LeafIndexCell> {
    let (i, payload_size) = be_u64_varint(i)?;
    let (i, payload) = index_cell_payload(i)?;

    Ok((
        i,
        LeafIndexCell {
            payload_size,
            payload,
            overflow_page_no: None,
        },
    ))
}

fn leaf_table_b_tree_page<const OFFSET: usize>(i: &[u8]) -> IResult<&[u8], LeafTablePage> {
    let (ii, _) = tag([0x0du8])(i)?;
    let (ii, header) = leaf_page_header(ii)?;
    let (ii, cell_pointers) = count(be_u16, header.no_cells.into())(ii)?;

    let mut cells = Vec::with_capacity(cell_pointers.len());
    for &ptr in cell_pointers.iter() {
        let (_, cell) = leaf_table_cell(&i[(ptr as usize - OFFSET)..])?;
        cells.push(cell);
    }

    Ok((
        ii,
        LeafTablePage {
            header,
            cell_pointers,
            cells,
        },
    ))
}

fn table_cell_payload(i: &[u8]) -> IResult<&[u8], TableCellPayload> {
    let (i, header_size) = be_u64_varint(i)?;
    let (_, column_types) = column_types(&i[0..header_size as usize - 1])?;
    let (i, column_values) = column_values(&i[header_size as usize - 1..], &column_types)?;

    Ok((
        i,
        TableCellPayload {
            header_size,
            column_types,
            column_values,
        },
    ))
}

fn leaf_table_cell(i: &[u8]) -> IResult<&[u8], LeafTableCell> {
    let (i, payload_size) = be_u64_varint(i)?;
    let (i, rowid) = be_u64_varint(i)?;
    let (i, payload) = table_cell_payload(i)?;

    Ok((
        i,
        LeafTableCell {
            payload_size,
            rowid,
            payload,
            overflow_page_no: None,
        },
    ))
}
