use crate::be_i48;
use be_i48::be_i48;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::take;
use nom::combinator::{complete, map, map_parser, map_res};
use nom::multi::{count, many0};
use nom::number::complete::{be_f64, be_i16, be_i24, be_i32, be_i64, be_i8, be_u16, be_u32, be_u8};
use nom::sequence::{pair, Tuple};
use nom::IResult;

use crate::model::*;
use crate::varint::be_u64_varint;

const HEADER_SIZE: usize = 100;

/// Goes through the whole input page-by-page
/// NOTE: you should use specific parsers or Reader to parse file lazily
pub fn database(i: &[u8]) -> IResult<&[u8], Database> {
    let (i, header) = db_header(i)?;

    let page_size = header.page_size.real_size();

    let root_page = map_parser(take(page_size - HEADER_SIZE), page_generic(HEADER_SIZE));
    let pages = complete(many0(map_parser(take(page_size), page_generic(0))));

    let (i, (root_page, mut pages)) = complete(pair(root_page, pages))(i)?;

    pages.insert(0, root_page);

    Ok((i, Database { header, pages }))
}

/// File header parser. Page size and text encoding are required for the rest to work correctly.
pub fn db_header(i: &[u8]) -> IResult<&[u8], DbHeader> {
    let (i, _) = tag("SQLite format 3\0")(i)?;
    let (i, page_size) = map(be_u16, PageSize)(i)?;
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
    let (i, db_text_encoding) = map_res(be_u32, |x| x.try_into())(i)?;
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

/// The page number 0, which comes right after the header. Input assumed to contain the header.
pub fn root_page(i: &[u8]) -> IResult<&[u8], Page> {
    let shrunk_page = &i[HEADER_SIZE..];
    page_generic(HEADER_SIZE)(shrunk_page)
}

/// All the rest of pages, pageno >0.
pub fn page(i: &[u8]) -> IResult<&[u8], Page> {
    page_generic(0)(i)
}

// todo: fix const generic thing, hack to pass through parameters
fn page_generic(page_start_offset: usize) -> impl FnMut(&[u8]) -> IResult<&[u8], Page> {
    move |i| {
        alt((
            map(
                interior_index_b_tree_page(page_start_offset),
                Page::InteriorIndex,
            ),
            map(leaf_index_b_tree_page(page_start_offset), Page::LeafIndex),
            map(
                interior_table_b_tree_page(page_start_offset),
                Page::InteriorTable,
            ),
            map(leaf_table_b_tree_page(page_start_offset), Page::LeafTable),
        ))(i)
    }
}

fn interior_page_header(i: &[u8]) -> IResult<&[u8], InteriorPageHeader> {
    let (i, first_freeblock_offset) = map(be_u16, |u| Some(u).filter(|&p| p != 0x0u16))(i)?;
    let (i, no_cells) = be_u16(i)?;
    let (i, cell_content_offset) = map(be_u16, CellOffset)(i)?;
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
    let (i, cell_content_offset) = map(be_u16, CellOffset)(i)?;
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

fn interior_index_b_tree_page(
    page_start_offset: usize,
) -> impl FnMut(&[u8]) -> IResult<&[u8], InteriorIndexPage> {
    move |i| {
        let (ii, _) = tag([0x02u8])(i)?;
        let (ii, header) = interior_page_header(ii)?;
        let (ii, cell_pointers) = count(be_u16, header.no_cells.into())(ii)?;

        let mut cells = Vec::with_capacity(cell_pointers.len());
        for &ptr in cell_pointers.iter() {
            let cell_offset = ptr as usize - page_start_offset;
            let (_, cell) = interior_index_cell(&i[cell_offset..])?;
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
}

/// Expects to get exactly as many bytes in input as it will consume
fn column_types(i: &[u8]) -> IResult<&[u8], Vec<SerialType>> {
    // many0 as header might actually be empty
    complete(many0(map(be_u64_varint, SerialType::from)))(i)
}

fn text_payload(size: usize) -> impl FnMut(&[u8]) -> IResult<&[u8], Option<Payload>> {
    move |i| map(take(size), |x: &[u8]| Some(Payload::Text(RawText::new(x))))(i)
}

fn blob_payload(size: usize) -> impl FnMut(&[u8]) -> IResult<&[u8], Option<Payload>> {
    move |i| map(take(size), |x: &[u8]| Some(Payload::Blob(x)))(i)
}

fn column_values<'a, 'b>(
    serial_types: &'b [SerialType],
) -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], Vec<Option<Payload>>> + 'b {
    move |i| {
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
                SerialType::Blob(_) => blob_payload(serial_type.size())(i),
                SerialType::Text(_) if serial_type.size() == 0 => Ok((i, None)),
                SerialType::Text(_) => text_payload(serial_type.size())(i),
            }?;
            i = ii;
            dbg!(v.clone());
            res.push(v);
        }

        Ok((i, res))
    }
}

fn index_cell_payload(i: &[u8]) -> IResult<&[u8], IndexCellPayload> {
    let (i, header_size) = be_u64_varint(i)?;
    let (_, column_types) = column_types(&i[0..header_size as usize - 1])?;
    let (i, column_values) = column_values(&column_types)(&i[header_size as usize - 1..])?;
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

fn interior_table_b_tree_page(
    page_start_offset: usize,
) -> impl FnMut(&[u8]) -> IResult<&[u8], InteriorTablePage> {
    move |i| {
        let (ii, _) = tag([0x05u8])(i)?;
        let (ii, header) = interior_page_header(ii)?;
        let (ii, cell_pointers) = count(be_u16, header.no_cells.into())(ii)?;

        let mut cells = Vec::with_capacity(cell_pointers.len());
        for &ptr in cell_pointers.iter() {
            let cell_offset = ptr as usize - page_start_offset;
            let (_, cell) = interior_table_cell(&i[cell_offset..])?;
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

fn leaf_index_b_tree_page(
    page_start_offset: usize,
) -> impl FnMut(&[u8]) -> IResult<&[u8], LeafIndexPage> {
    move |i| {
        let (ii, _) = tag([0x0au8])(i)?;
        let (ii, header) = leaf_page_header(ii)?;
        let (ii, cell_pointers) = count(be_u16, header.no_cells.into())(ii)?;

        let mut cells = Vec::with_capacity(cell_pointers.len());
        for &ptr in cell_pointers.iter() {
            let cell_offset = ptr as usize - page_start_offset;
            let (_, cell) = leaf_index_cell(&i[cell_offset..])?;
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

fn leaf_table_b_tree_page(
    page_start_offset: usize,
) -> impl FnMut(&[u8]) -> IResult<&[u8], LeafTablePage> {
    move |i| {
        let (ii, _) = tag([0x0du8])(i)?;
        let (ii, header) = leaf_page_header(ii)?;
        let (ii, cell_pointers) = count(be_u16, header.no_cells.into())(ii)?;

        let mut cells = Vec::with_capacity(cell_pointers.len());
        for &ptr in cell_pointers.iter() {
            let cell_offset = ptr as usize - page_start_offset;
            let (_, cell) = leaf_table_cell(&i[cell_offset..])?;
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
}

fn table_cell_payload(i: &[u8]) -> IResult<&[u8], TableCellPayload> {
    let (i, header_size) = be_u64_varint(i)?;
    let (_, column_types) = column_types(&i[0..header_size as usize - 1])?;
    let (i, column_values) = column_values(&column_types)(&i[header_size as usize - 1..])?;

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
