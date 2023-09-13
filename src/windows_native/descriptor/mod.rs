mod typedefs;
mod types;
mod encoder;
#[cfg(test)]
mod tests;


use std::collections::HashMap;
use std::ffi::c_void;
use std::slice;
use crate::ensure;
use crate::windows_native::descriptor::encoder::encode_descriptor;
use crate::windows_native::descriptor::typedefs::{Caps, HidpPreparsedData, LinkCollectionNode};
use crate::windows_native::descriptor::types::{BitRange, ItemNodeType, MainItemNode, MainItems, ReportType};
use crate::windows_native::error::{WinError, WinResult};
use crate::windows_native::hid::PreparsedData;
use crate::windows_native::utils::PeakIterExt;


pub fn get_descriptor(pp_data: &PreparsedData) -> WinResult<Vec<u8>> {
    unsafe { get_descriptor_ptr(pp_data.as_ptr()) }
}

unsafe fn get_descriptor_ptr(pp_data: *const c_void) -> WinResult<Vec<u8>> {
    let (header, caps_list, link_collection_nodes) = extract_structures(pp_data)?;

    let list = reconstruct_descriptor(header, caps_list, link_collection_nodes);

    encode_descriptor(&list, caps_list, link_collection_nodes)
}


unsafe fn extract_structures<'a>(pp_data: *const c_void) -> WinResult<(HidpPreparsedData, &'a [Caps], &'a [LinkCollectionNode])> {
    let header: *const HidpPreparsedData = pp_data as _;

    // Check if MagicKey is correct, to ensure that pp_data points to an valid preparse data structure
    ensure!(&(*header).magic_key == b"HidP KDR", Err(WinError::InvalidPreparsedData));

    let caps_ptr: *const Caps = header.offset(1) as _;
    let caps_len = ReportType::values()
        .into_iter()
        .map(|r| (*header).caps_info[r as usize].last_cap)
        .max()
        .unwrap() as usize;

    let link_ptr: *const LinkCollectionNode = ((caps_ptr as *const c_void)
        .offset((*header).first_byte_of_link_collection_array as isize)) as _;
    let link_len = (*header).number_link_collection_nodes as usize;

    Ok((*header, slice::from_raw_parts(caps_ptr, caps_len), slice::from_raw_parts(link_ptr, link_len)))
}

fn reconstruct_descriptor(header: HidpPreparsedData, caps_list: &[Caps], link_collection_nodes: &[LinkCollectionNode]) -> Vec<MainItemNode> {
    // ****************************************************************************************************************************
    // Create lookup tables for the bit range of each report per collection (position of first bit and last bit in each collection)
    // coll_bit_range[COLLECTION_INDEX][REPORT_ID][INPUT/OUTPUT/FEATURE]
    // ****************************************************************************************************************************
    let mut coll_bit_range: HashMap<(usize, u8, ReportType), BitRange> = HashMap::new();
    for rt_idx in ReportType::values() {
        let caps_info = header.caps_info[rt_idx as usize];
        for caps_idx in caps_info.first_cap..caps_info.last_cap {
            let caps = caps_list[caps_idx as usize];
            let range = caps.get_bit_range();
            coll_bit_range
                .entry((caps.link_collection as usize, caps.report_id, rt_idx))
                .and_modify(|r| *r = r.merge(range))
                .or_insert(range);
        }
    }

    // *************************************************************************
    // -Determine hierachy levels of each collections and store it in:
    //  coll_levels[COLLECTION_INDEX]
    // -Determine number of direct childs of each collections and store it in:
    //  coll_number_of_direct_childs[COLLECTION_INDEX]
    // *************************************************************************
    let mut max_coll_level = 0;
    let mut coll_levels = vec![-1; link_collection_nodes.len()];
    let mut coll_number_of_direct_childs = vec![0; link_collection_nodes.len()];
    {
        let mut actual_coll_level = 0;
        let mut collection_node_idx = 0;
        while actual_coll_level >= 0 {
            coll_levels[collection_node_idx] = actual_coll_level;
            let node = link_collection_nodes[collection_node_idx];
            if node.number_of_children > 0 && coll_levels[node.first_child as usize] == -1 {
                actual_coll_level += 1;
                coll_levels[collection_node_idx] = actual_coll_level;
                max_coll_level = max_coll_level.max(actual_coll_level);
                coll_number_of_direct_childs[collection_node_idx] += 1;
                collection_node_idx = node.first_child as usize;
            } else if node.next_sibling != 0 {
                coll_number_of_direct_childs[node.parent as usize] += 1;
                collection_node_idx = node.next_sibling as usize;
            } else {
                actual_coll_level -= 1;
                if actual_coll_level >= 0 {
                    collection_node_idx = node.parent as usize;
                }
            }
        }
    }

    // *********************************************************************************
    // Propagate the bit range of each report from the child collections to their parent
    // and store the merged result for the parent
    // *********************************************************************************
    for actual_coll_level in (0..max_coll_level).rev() {
        for collection_node_idx in 0..link_collection_nodes.len() {
            if coll_levels[collection_node_idx] == actual_coll_level {
                let mut child_idx = link_collection_nodes[collection_node_idx].first_child as usize;
                while child_idx != 0 {
                    for reportid_idx in 0..=255 {
                        for rt_idx in ReportType::values() {
                            if let Some(child) = coll_bit_range.get(&(child_idx, reportid_idx, rt_idx)).copied() {
                                coll_bit_range
                                    .entry((collection_node_idx, reportid_idx, rt_idx))
                                    .and_modify(|r| *r = r.merge(child))
                                    .or_insert(child);
                            }
                            child_idx = link_collection_nodes[child_idx].next_sibling as usize;
                        }
                    }
                }
            }
        }
    }

    // *************************************************************************************************
    // Determine child collection order of the whole hierachy, based on previously determined bit ranges
    // and store it this index coll_child_order[COLLECTION_INDEX][DIRECT_CHILD_INDEX]
    // *************************************************************************************************
    let mut coll_child_order: HashMap<(usize, u16), usize> = HashMap::new();
    {
        let mut coll_parsed_flag = vec![false; link_collection_nodes.len()];
        let mut actual_coll_level = 0;
        let mut collection_node_idx = 0;
        while actual_coll_level >= 0 {
            if coll_number_of_direct_childs[collection_node_idx] != 0 &&
                !coll_parsed_flag[link_collection_nodes[collection_node_idx].first_child as usize] {
                coll_parsed_flag[link_collection_nodes[collection_node_idx].first_child as usize] = true;

                {
                    // Create list of child collection indices
                    // sorted reverse to the order returned to HidP_GetLinkCollectionNodeschild
                    // which seems to match teh original order, as long as no bit position needs to be considered
                    let mut child_idx = link_collection_nodes[collection_node_idx].first_child as usize;
                    let mut child_count = coll_number_of_direct_childs[collection_node_idx] - 1;
                    coll_child_order.insert((collection_node_idx, child_count), child_idx);
                    while link_collection_nodes[child_idx].next_sibling != 0 {
                        child_count -= 1;
                        child_idx = link_collection_nodes[child_idx].next_sibling as usize;
                        coll_child_order.insert((collection_node_idx, child_count), child_idx);
                    }
                }

                if coll_number_of_direct_childs[collection_node_idx] > 1 {
                    // Sort child collections indices by bit positions
                    for rt_idx in ReportType::values() {
                        for report_idx in 0..=255 {
                            for child_idx in 1..coll_number_of_direct_childs[collection_node_idx] {
                                // since the coll_bit_range array is not sorted, we need to reference the collection index in
                                // our sorted coll_child_order array, and look up the corresponding bit ranges for comparing values to sort
                                let prev_coll_idx = *coll_child_order
                                    .get(&(collection_node_idx, child_idx - 1))
                                    .unwrap();
                                let cur_coll_idx = *coll_child_order
                                    .get(&(collection_node_idx, child_idx))
                                    .unwrap();
                                let swap = coll_bit_range
                                    .get(&(prev_coll_idx, report_idx, rt_idx))
                                    .map(|prev| prev.first_bit)
                                    .zip(coll_bit_range
                                        .get(&(cur_coll_idx, report_idx, rt_idx))
                                        .map(|prev| prev.first_bit))
                                    .map_or(false, |(prev, cur)| prev > cur);
                                if swap {
                                    coll_child_order.insert((collection_node_idx, (child_idx - 1)), cur_coll_idx);
                                    coll_child_order.insert((collection_node_idx, child_idx), prev_coll_idx);
                                }
                            }
                        }
                    }
                }
                actual_coll_level += 1;
                collection_node_idx = link_collection_nodes[collection_node_idx].first_child as usize;
            } else if link_collection_nodes[collection_node_idx].next_sibling != 0 {
                collection_node_idx = link_collection_nodes[collection_node_idx].next_sibling as usize;
            } else {
                actual_coll_level -= 1;
                if actual_coll_level >= 0 {
                    collection_node_idx = link_collection_nodes[collection_node_idx].parent as usize;
                }
            }
        }
    }

    // ***************************************************************************************
    // Create sorted main_item_list containing all the Collection and CollectionEnd main items
    // ***************************************************************************************
    let mut main_item_list: Vec<MainItemNode> = Vec::new();
    {
        let mut coll_last_written_child  = vec![-1i32; link_collection_nodes.len()];

        let mut actual_coll_level = 0;
        let mut collection_node_idx = 0;
        let mut first_delimiter_node = false;

        main_item_list.push(MainItemNode::new(0, 0, ItemNodeType::Collection, 0, collection_node_idx, MainItems::Collection, 0));
        while actual_coll_level >= 0 {
            if coll_number_of_direct_childs[collection_node_idx] != 0 && coll_last_written_child[collection_node_idx] == -1 {
                // Collection has child collections, but none is written to the list yet
                coll_last_written_child[collection_node_idx] = coll_child_order[&(collection_node_idx, 0)] as i32;
                collection_node_idx = coll_child_order[&(collection_node_idx, 0)];

                // In a HID Report Descriptor, the first usage declared is the most preferred usage for the control.
                // While the order in the WIN32 capabiliy strutures is the opposite:
                // Here the preferred usage is the last aliased usage in the sequence.
                if link_collection_nodes[collection_node_idx].is_alias() && !first_delimiter_node {
                    first_delimiter_node = true;
                    main_item_list.push(MainItemNode::new(0, 0, ItemNodeType::Collection, 0, collection_node_idx, MainItems::DelimiterUsage, 0));
                    main_item_list.push(MainItemNode::new(0, 0, ItemNodeType::Collection, 0, collection_node_idx, MainItems::DelimiterClose, 0));
                } else {
                    main_item_list.push(MainItemNode::new(0, 0, ItemNodeType::Collection, 0, collection_node_idx, MainItems::Collection, 0));
                    actual_coll_level += 1;
                }
            } else if coll_number_of_direct_childs[collection_node_idx] > 1 &&
                coll_last_written_child[collection_node_idx] != coll_child_order[&(collection_node_idx, coll_number_of_direct_childs[collection_node_idx] - 1)] as i32 {

                // Collection has child collections, and this is not the first child
                let mut next_child = 1;
                while coll_last_written_child[collection_node_idx] != coll_child_order[&(collection_node_idx, (next_child - 1))] as i32 {
                    next_child += 1;
                }
                coll_last_written_child[collection_node_idx] = coll_child_order[&(collection_node_idx, next_child)] as i32;
                collection_node_idx = coll_child_order[&(collection_node_idx, next_child)];

                if link_collection_nodes[collection_node_idx].is_alias() && !first_delimiter_node {
                    // Alliased Collection (First node in link_collection_nodes -> Last entry in report descriptor output)
                    first_delimiter_node = true;
                    main_item_list.push(MainItemNode::new(0, 0, ItemNodeType::Collection, 0, collection_node_idx, MainItems::DelimiterUsage, 0));
                    main_item_list.push(MainItemNode::new(0, 0, ItemNodeType::Collection, 0, collection_node_idx, MainItems::DelimiterClose, 0));
                } else if link_collection_nodes[collection_node_idx].is_alias() && first_delimiter_node {
                    main_item_list.insert(1, MainItemNode::new(0, 0, ItemNodeType::Collection, 0, collection_node_idx, MainItems::DelimiterUsage, 0));
                } else if !link_collection_nodes[collection_node_idx].is_alias() && first_delimiter_node {
                    main_item_list.insert(1, MainItemNode::new(0, 0, ItemNodeType::Collection, 0, collection_node_idx, MainItems::DelimiterUsage, 0));
                    main_item_list.insert(1, MainItemNode::new(0, 0, ItemNodeType::Collection, 0, collection_node_idx, MainItems::DelimiterClose, 0));
                    first_delimiter_node = false;
                }
                if !link_collection_nodes[collection_node_idx].is_alias() {
                    main_item_list.push(MainItemNode::new(0, 0, ItemNodeType::Collection, 0, collection_node_idx, MainItems::Collection, 0));
                    actual_coll_level += 1;
                }
            } else {
                actual_coll_level -= 1;
                main_item_list.push(MainItemNode::new(0, 0, ItemNodeType::Collection, 0, collection_node_idx, MainItems::CollectionEnd, 0));
                collection_node_idx = link_collection_nodes[collection_node_idx].parent as usize;
            }
        }
    }

    // ****************************************************************
    // Inserted Input/Output/Feature main items into the main_item_list
    // in order of reconstructed bit positions
    // ****************************************************************
    for rt_idx in ReportType::values() {
        // Add all value caps to node list
        let mut first_delimiter_node = false;
        let caps_info = header.caps_info[rt_idx as usize];
        for caps_idx in caps_info.first_cap..caps_info.last_cap {
            let caps = caps_list[caps_idx as usize];
            let mut coll_begin = main_item_list
                .iter()
                .position(|node| node.collection_index == caps.link_collection as usize)
                .unwrap();
            let (first_bit, last_bit) = {
                let range = caps.get_bit_range();
                (range.first_bit, range.last_bit)
            };

            for child_idx in 0..coll_number_of_direct_childs[caps.link_collection as usize] {
                // Determine in which section before/between/after child collection the item should be inserted
                let child_first_bit = coll_child_order
                    .get(&(caps.link_collection as usize, child_idx))
                    .and_then(|i| coll_bit_range.get(&(*i, caps.report_id, rt_idx)))
                    .map(|r| r.first_bit)
                    .unwrap_or(0);
                if first_bit < child_first_bit {
                    // Note, that the default value for undefined coll_bit_range is -1, which can't be greater than the bit position
                    break;
                }
                let index = coll_child_order[&(caps.link_collection as usize, child_idx)];
                coll_begin = main_item_list
                    .iter()
                    .rposition(|node| node.collection_index == index)
                    .unwrap();
            }
            let list_node = 1 + search_list(first_bit as i32, rt_idx.into(), caps.report_id, coll_begin, &main_item_list);

            // In a HID Report Descriptor, the first usage declared is the most preferred usage for the control.
            // While the order in the WIN32 capabiliy strutures is the opposite:
            // Here the preferred usage is the last aliased usage in the sequence.

            if caps.is_alias() && !first_delimiter_node {
                // Alliased Usage (First node in pp_data->caps -> Last entry in report descriptor output)
                first_delimiter_node = true;
                main_item_list.insert(list_node, MainItemNode::new(first_bit, last_bit, ItemNodeType::Cap, caps_idx as i32, caps.link_collection as usize, MainItems::DelimiterUsage, caps.report_id));
                main_item_list.insert(list_node, MainItemNode::new(first_bit, last_bit, ItemNodeType::Cap, caps_idx as i32, caps.link_collection as usize, MainItems::DelimiterClose, caps.report_id));
            } else if caps.is_alias() && first_delimiter_node {
                main_item_list.insert(list_node, MainItemNode::new(first_bit, last_bit, ItemNodeType::Cap, caps_idx as i32, caps.link_collection as usize, MainItems::DelimiterUsage, caps.report_id));
            } else if !caps.is_alias() && first_delimiter_node {
                // Alliased Collection (Last node in pp_data->caps -> First entry in report descriptor output)
                main_item_list.insert(list_node, MainItemNode::new(first_bit, last_bit, ItemNodeType::Cap, caps_idx as i32, caps.link_collection as usize, MainItems::DelimiterUsage, caps.report_id));
                main_item_list.insert(list_node, MainItemNode::new(first_bit, last_bit, ItemNodeType::Cap, caps_idx as i32, caps.link_collection as usize, MainItems::DelimiterOpen, caps.report_id));
                first_delimiter_node = false;
            }
            if !caps.is_alias() {
                main_item_list.insert(list_node, MainItemNode::new(first_bit, last_bit, ItemNodeType::Cap, caps_idx as i32, caps.link_collection as usize, rt_idx.into(), caps.report_id));
            }
        }
    }

    // ***********************************************************
    // Add const main items for padding to main_item_list
    // -To fill all bit gaps
    // -At each report end for 8bit padding
    //  Note that information about the padding at the report end,
    //  is not stored in the preparsed data, but in practice all
    //  report descriptors seem to have it, as assumed here.
    // ***********************************************************
    {
        let mut last_bit_position: HashMap<(MainItems, u8), i32> = HashMap::new();
        let mut last_report_item_lookup: HashMap<(MainItems, u8), usize> = HashMap::new();

        let mut index = 0;
        while index < main_item_list.len() {
            let current = main_item_list[index];
            if ReportType::try_from(current.main_item_type).is_ok() {
                let lbp = last_bit_position
                    .get(&(current.main_item_type, current.report_id))
                    .copied()
                    .unwrap_or(-1);
                let lrip = last_report_item_lookup
                    .get(&(current.main_item_type, current.report_id))
                    .copied();
                if lbp + 1 != current.first_bit as i32 && lrip.is_some_and(|i|
                        main_item_list[i].first_bit != current.first_bit) {
                    let list_node = search_list(lbp, current.main_item_type, current.report_id, lrip.unwrap(), &main_item_list);
                    main_item_list.insert(list_node + 1, MainItemNode::new((lbp + 1) as u16, current.first_bit - 1, ItemNodeType::Padding, -1, 0, current.main_item_type, current.report_id));
                    index += 1;
                }
                last_bit_position.insert((current.main_item_type, current.report_id), current.last_bit as i32);
                last_report_item_lookup.insert((current.main_item_type, current.report_id), index);
            }
            index += 1;
        }

        for rt_idx in ReportType::values() {
            for report_idx in 0..=255 {
                if let Some(lbp) = last_bit_position.get(&(rt_idx.into(), report_idx)) {
                    let padding = 8 - ((*lbp + 1) % 8);
                    if padding < 8 {
                        // Insert padding item after item referenced in last_report_item_lookup
                        let lrip = *last_report_item_lookup.get(&(rt_idx.into(), report_idx)).unwrap();
                        main_item_list.insert(lrip + 1, MainItemNode::new((lbp + 1) as u16, (lbp + padding) as u16, ItemNodeType::Padding, -1, 0, rt_idx.into(), report_idx));
                        last_report_item_lookup
                            .values_mut()
                            .filter(|i| **i > lrip)
                            .for_each(|i| *i += 1);
                    }
                }
            }
        }
    }
    main_item_list
}

fn search_list(search_bit: i32, main_item_type: MainItems, report_id: u8, start: usize, list: &[MainItemNode]) -> usize {
    list[start..]
        .iter()
        .peaking()
        .position(|(_, next)| next.is_some_and(|next| next.main_item_type == MainItems::Collection ||
           next.main_item_type == MainItems::CollectionEnd ||
           (next.last_bit as i32 >= search_bit && next.report_id == report_id && next.main_item_type == main_item_type) ))
        .unwrap() + start
}
