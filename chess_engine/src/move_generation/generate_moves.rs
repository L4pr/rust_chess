use crate::board::board::Board;
use crate::board::move_struct::Move;
use crate::board::pieces::*;
use crate::move_generation::magic_bitboards::{get_rook_attacks, get_bishop_attacks, get_queen_attacks};
pub fn generate_all_moves(board: &Board, moves: &mut [Move]) -> usize {
    let mut curr_move_index = 0;

    generate_pawn_moves(board, moves, &mut curr_move_index);
    generate_knight_moves(board, moves, &mut curr_move_index);
    generate_bishop_moves(board, moves, &mut curr_move_index);
    generate_rook_moves(board, moves, &mut curr_move_index);
    generate_queen_moves(board, moves, &mut curr_move_index);
    generate_king_moves(board, moves, &mut curr_move_index);

    curr_move_index
}

fn generate_pawn_moves(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let pawns = board.pieces[Piece::new(us, Piece::PAWN).0 as usize];
    let empty = !board.pieces[0];
    let enemy_occ = board.pieces[((us ^ 8) | Piece::ALL) as usize];

    let rank_4 = if board.white_to_move { 0x00000000FF000000u64 } else { 0x000000FF00000000u64 };
    let promo_rank = if board.white_to_move { 0xFF00000000000000u64 } else { 0x00000000000000FFu64 };

    // --- Single pushes ---
    let single = if board.white_to_move { pawns << 8 } else { pawns >> 8 } & empty;

    // Promotions
    let mut promos = single & promo_rank;
    while promos != 0 {
        let to = promos.trailing_zeros() as u8;
        let from = if board.white_to_move { to - 8 } else { to + 8 };
        push_promotions(moves, curr_move_index, from, to, false);
        promos &= promos - 1;
    }

    // Quiet single pushes (non-promotion)
    let mut quiet = single & !promo_rank;
    while quiet != 0 {
        let to = quiet.trailing_zeros() as u8;
        let from = if board.white_to_move { to - 8 } else { to + 8 };
        moves[*curr_move_index] = Move::new_with_flags(from, to, Move::QUIET);
        *curr_move_index += 1;
        quiet &= quiet - 1;
    }

    // --- Double pushes ---
    let double = if board.white_to_move {
        (single << 8) & empty & rank_4
    } else {
        (single >> 8) & empty & rank_4
    };
    let mut dbl = double;
    while dbl != 0 {
        let to = dbl.trailing_zeros() as u8;
        let from = if board.white_to_move { to - 16 } else { to + 16 };
        moves[*curr_move_index] = Move::new_with_flags(from, to, Move::DOUBLE_PAWN_PUSH);
        *curr_move_index += 1;
        dbl &= dbl - 1;
    }

    // --- Captures ---
    let (left_shift, right_shift): (fn(u64) -> u64, fn(u64) -> u64) = if board.white_to_move {
        (|b| (b << 7) & NOT_H_FILE, |b| (b << 9) & NOT_A_FILE)
    } else {
        (|b| (b >> 9) & NOT_H_FILE, |b| (b >> 7) & NOT_A_FILE)
    };

    // Left captures
    let mut left_caps = left_shift(pawns) & enemy_occ;
    let mut left_promos = left_caps & promo_rank;
    left_caps &= !promo_rank;

    while left_promos != 0 {
        let to = left_promos.trailing_zeros() as u8;
        let from = if board.white_to_move { to - 7 } else { to + 9 };
        push_promotions(moves, curr_move_index, from, to, true);
        left_promos &= left_promos - 1;
    }
    while left_caps != 0 {
        let to = left_caps.trailing_zeros() as u8;
        let from = if board.white_to_move { to - 7 } else { to + 9 };
        moves[*curr_move_index] = Move::new_with_flags(from, to, Move::CAPTURE);
        *curr_move_index += 1;
        left_caps &= left_caps - 1;
    }

    // Right captures
    let mut right_caps = right_shift(pawns) & enemy_occ;
    let mut right_promos = right_caps & promo_rank;
    right_caps &= !promo_rank;

    while right_promos != 0 {
        let to = right_promos.trailing_zeros() as u8;
        let from = if board.white_to_move { to - 9 } else { to + 7 };
        push_promotions(moves, curr_move_index, from, to, true);
        right_promos &= right_promos - 1;
    }
    while right_caps != 0 {
        let to = right_caps.trailing_zeros() as u8;
        let from = if board.white_to_move { to - 9 } else { to + 7 };
        moves[*curr_move_index] = Move::new_with_flags(from, to, Move::CAPTURE);
        *curr_move_index += 1;
        right_caps &= right_caps - 1;
    }

    // --- En passant ---
    if let Some(ep_sq) = board.en_passant_square {
        let ep_bit = 1u64 << ep_sq;
        let left_ep = left_shift(pawns) & ep_bit;
        if left_ep != 0 {
            let from = if board.white_to_move { ep_sq - 7 } else { ep_sq + 9 };
            moves[*curr_move_index] = Move::new_with_flags(from, ep_sq, Move::EN_PASSANT);
            *curr_move_index += 1;
        }
        let right_ep = right_shift(pawns) & ep_bit;
        if right_ep != 0 {
            let from = if board.white_to_move { ep_sq - 9 } else { ep_sq + 7 };
            moves[*curr_move_index] = Move::new_with_flags(from, ep_sq, Move::EN_PASSANT);
            *curr_move_index += 1;
        }
    }
}


fn push_promotions(moves: &mut [Move], index: &mut usize, from: u8, to: u8, capture: bool) {
    let flags = if capture {
        [Move::PC_QUEEN, Move::PC_ROOK, Move::PC_BISHOP, Move::PC_KNIGHT]
    } else {
        [Move::PR_QUEEN, Move::PR_ROOK, Move::PR_BISHOP, Move::PR_KNIGHT]
    };

    for &flag in &flags {
        moves[*index] = Move::new_with_flags(from, to, flag);
        *index += 1;
    }
}

fn generate_knight_moves(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let friendly_occ = board.pieces[(us | Piece::ALL) as usize];
    let enemy_occ = board.pieces[((us ^ 8) | Piece::ALL) as usize];
    let mut knights = board.pieces[(us | Piece::KNIGHT) as usize];

    while knights != 0 {
        let from = knights.trailing_zeros() as u8;
        let mut attacks = KNIGHT_MOVES[from as usize] & !friendly_occ;

        while attacks != 0 {
            let to = attacks.trailing_zeros() as u8;
            let flag = if ((1u64 << to) & enemy_occ) != 0 { Move::CAPTURE } else { Move::QUIET };
            moves[*curr_move_index] = Move::new_with_flags(from, to, flag);
            *curr_move_index += 1;
            attacks &= attacks - 1;
        }
        knights &= knights - 1;
    }
}

fn generate_bishop_moves(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let enemy = us ^ 8;
    let mut bishops = board.pieces[(us | Piece::BISHOP) as usize];

    let friendly_occ = board.pieces[(us | Piece::ALL) as usize];
    let enemy_occ = board.pieces[(enemy | Piece::ALL) as usize];
    let total_occ = board.pieces[0];

    while bishops != 0 {
        let from = bishops.trailing_zeros() as u8;

        // Use magic bitboards to get all attacks
        let mut attacks = get_bishop_attacks(from, total_occ) & !friendly_occ;

        while attacks != 0 {
            let to = attacks.trailing_zeros() as u8;
            let to_bit = 1u64 << to;

            let flag = if (to_bit & enemy_occ) != 0 {
                Move::CAPTURE
            } else {
                Move::QUIET
            };

            moves[*curr_move_index] = Move::new_with_flags(from, to, flag);
            *curr_move_index += 1;

            attacks &= attacks - 1;
        }

        bishops &= bishops - 1;
    }
}

fn generate_rook_moves(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let enemy = us ^ 8;
    let mut rooks = board.pieces[(us | Piece::ROOK) as usize];

    let friendly_occ = board.pieces[(us | Piece::ALL) as usize];
    let enemy_occ = board.pieces[(enemy | Piece::ALL) as usize];
    let total_occ = board.pieces[0];

    while rooks != 0 {
        let from = rooks.trailing_zeros() as u8;

        // Use magic bitboards to get all attacks
        let mut attacks = get_rook_attacks(from, total_occ) & !friendly_occ;

        while attacks != 0 {
            let to = attacks.trailing_zeros() as u8;
            let to_bit = 1u64 << to;

            let flag = if (to_bit & enemy_occ) != 0 {
                Move::CAPTURE
            } else {
                Move::QUIET
            };

            moves[*curr_move_index] = Move::new_with_flags(from, to, flag);
            *curr_move_index += 1;

            attacks &= attacks - 1;
        }

        rooks &= rooks - 1;
    }
}

fn generate_queen_moves(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let enemy = us ^ 8;
    let mut queens = board.pieces[(us | Piece::QUEEN) as usize];

    let friendly_occ = board.pieces[(us | Piece::ALL) as usize];
    let enemy_occ = board.pieces[(enemy | Piece::ALL) as usize];
    let total_occ = board.pieces[0];

    while queens != 0 {
        let from = queens.trailing_zeros() as u8;

        // Use magic bitboards to get all attacks (combination of rook and bishop)
        let mut attacks = get_queen_attacks(from, total_occ) & !friendly_occ;

        while attacks != 0 {
            let to = attacks.trailing_zeros() as u8;
            let to_bit = 1u64 << to;

            let flag = if (to_bit & enemy_occ) != 0 {
                Move::CAPTURE
            } else {
                Move::QUIET
            };

            moves[*curr_move_index] = Move::new_with_flags(from, to, flag);
            *curr_move_index += 1;

            attacks &= attacks - 1;
        }

        queens &= queens - 1;
    }
}

fn generate_king_moves(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let enemy = us ^ 8;

    let total_occ = board.pieces[0];

    // Get King's square index
    let king_bb = board.pieces[(us | Piece::KING) as usize];
    if king_bb == 0 { return; }
    let from = king_bb.trailing_zeros() as u8;

    // --- 1. BASIC JUMPS ---
    // Use the precomputed lookup table and mask out squares we already occupy
    let mut attacks = KING_MOVES[from as usize] & !board.pieces[(us | Piece::ALL) as usize];

    while attacks != 0 {
        let to = attacks.trailing_zeros() as u8;
        let to_bit = 1u64 << to;

        let flag = if (to_bit & board.pieces[(enemy | Piece::ALL) as usize]) != 0 { Move::CAPTURE } else { Move::QUIET };
        moves[*curr_move_index] = Move::new_with_flags(from, to, flag);
        *curr_move_index += 1;

        attacks &= attacks - 1;
    }

    // --- 2. CASTLING ---
    // Rules: King hasn't moved, Rook hasn't moved, path is clear,
    // AND (crucially) squares are not under attack.
    if board.white_to_move {
        // Kingside (White)
        if board.castling_rights.white_kingside() {
            // Squares between King (e1) and Rook (h1) must be empty: f1 (bit 5), g1 (bit 6)
            let path_mask = (1u64 << 5) | (1u64 << 6);
            if (total_occ & path_mask) == 0 {
                // Must ensure King is not in check, and doesn't pass through check
                if !is_square_attacked(board, 4, enemy) &&
                    !is_square_attacked(board, 5, enemy) &&
                    !is_square_attacked(board, 6, enemy) {
                    moves[*curr_move_index] = Move::new_with_flags(4, 6, Move::KING_CASTLE);
                    *curr_move_index += 1;
                }
            }
        }
        // Queenside (White)
        if board.castling_rights.white_queenside() {
            // Squares between King (e1) and Rook (a1) must be empty: b1 (1), c1 (2), d1 (3)
            let path_mask = (1u64 << 1) | (1u64 << 2) | (1u64 << 3);
            if (total_occ & path_mask) == 0 {
                // Note: b1 doesn't need to be safe from attack, only c1, d1, and e1
                if !is_square_attacked(board, 4, enemy) &&
                    !is_square_attacked(board, 3, enemy) &&
                    !is_square_attacked(board, 2, enemy) {
                    moves[*curr_move_index] = Move::new_with_flags(4, 2, Move::QUEEN_CASTLE);
                    *curr_move_index += 1;
                }
            }
        }
    } else {
        // --- BLACK CASTLING ---
        // Kingside (Black): f8 (61), g8 (62)
        if board.castling_rights.black_kingside() {
            let path_mask = (1u64 << 61) | (1u64 << 62);
            if (total_occ & path_mask) == 0 {
                if !is_square_attacked(board, 60, enemy) &&
                    !is_square_attacked(board, 61, enemy) &&
                    !is_square_attacked(board, 62, enemy) {
                    moves[*curr_move_index] = Move::new_with_flags(60, 62, Move::KING_CASTLE);
                    *curr_move_index += 1;
                }
            }
        }
        // Queenside (Black): b8 (57), c8 (58), d8 (59)
        if board.castling_rights.black_queenside() {
            let path_mask = (1u64 << 57) | (1u64 << 58) | (1u64 << 59);
            if (total_occ & path_mask) == 0 {
                if !is_square_attacked(board, 60, enemy) &&
                    !is_square_attacked(board, 59, enemy) &&
                    !is_square_attacked(board, 58, enemy) {
                    moves[*curr_move_index] = Move::new_with_flags(60, 58, Move::QUEEN_CASTLE);
                    *curr_move_index += 1;
                }
            }
        }
    }
}

#[inline]
pub fn is_square_attacked(board: &Board, sq: u8, attacker_color: u8) -> bool {
    // 1. KNIGHT Attacks
    // Reciprocal logic: If a Knight on 'sq' can hit an enemy Knight,
    // then that enemy Knight is attacking 'sq'.
    let enemy_knights = board.pieces[(attacker_color | Piece::KNIGHT) as usize];
    if (KNIGHT_MOVES[sq as usize] & enemy_knights) != 0 { return true; }

    // 2. KING Attacks
    let enemy_king = board.pieces[(attacker_color | Piece::KING) as usize];
    if (KING_MOVES[sq as usize] & enemy_king) != 0 { return true; }

    // 3. PAWN Attacks
    let enemy_pawns = board.pieces[(attacker_color | Piece::PAWN) as usize];
    let sq_bit = 1u64 << sq;
    // We look "backwards" from the square to see if a pawn could be there.
    // If we are checking if WHITE is attacking, we look where BLACK pawns would come from.
    let pawn_attacks = if attacker_color == Piece::WHITE {
        // White pawns attack "up" (from lower ranks to higher)
        ((sq_bit >> 7) & NOT_A_FILE) | ((sq_bit >> 9) & NOT_H_FILE)
    } else {
        // Black pawns attack "down"
        ((sq_bit << 7) & NOT_H_FILE) | ((sq_bit << 9) & NOT_A_FILE)
    };
    if (pawn_attacks & enemy_pawns) != 0 { return true; }

    // 4. SLIDING Attacks (Bishops, Rooks, Queens)
    let occupancy = board.pieces[0]; // All pieces on board block rays

    // --- Bishop/Queen Attacks (Diagonals) ---
    let enemy_diag = board.pieces[(attacker_color | Piece::BISHOP) as usize] |
        board.pieces[(attacker_color | Piece::QUEEN) as usize];
    if (get_bishop_attacks(sq, occupancy) & enemy_diag) != 0 { return true; }

    // --- Rook/Queen Attacks (Orthogonals) ---
    let enemy_ortho = board.pieces[(attacker_color | Piece::ROOK) as usize] |
        board.pieces[(attacker_color | Piece::QUEEN) as usize];
    if (get_rook_attacks(sq, occupancy) & enemy_ortho) != 0 { return true; }

    false
}

// Note: get_bishop_attacks and get_rook_attacks are now imported from magic_bitboards module

// --- HIGH SPEED CAPTURE GENERATOR FOR QUIESCENCE SEARCH ---

pub fn generate_captures(board: &Board, moves: &mut [Move]) -> usize {
    let mut curr_move_index = 0;

    generate_pawn_captures(board, moves, &mut curr_move_index);
    generate_knight_captures(board, moves, &mut curr_move_index);
    generate_bishop_captures(board, moves, &mut curr_move_index);
    generate_rook_captures(board, moves, &mut curr_move_index);
    generate_queen_captures(board, moves, &mut curr_move_index);
    generate_king_captures(board, moves, &mut curr_move_index);

    curr_move_index
}

fn generate_pawn_captures(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let pawns = board.pieces[Piece::new(us, Piece::PAWN).0 as usize];
    let empty = !board.pieces[0];
    let enemy_occ = board.pieces[((us ^ 8) | Piece::ALL) as usize];
    let promo_rank = if board.white_to_move { 0xFF00000000000000u64 } else { 0x00000000000000FFu64 };

    // Promotion pushes (capture-only gen still includes these since they're tactical)
    let single = if board.white_to_move { pawns << 8 } else { pawns >> 8 } & empty;
    let mut promos = single & promo_rank;
    while promos != 0 {
        let to = promos.trailing_zeros() as u8;
        let from = if board.white_to_move { to - 8 } else { to + 8 };
        push_promotions(moves, curr_move_index, from, to, false);
        promos &= promos - 1;
    }

    // Captures
    let (left_shift, right_shift): (fn(u64) -> u64, fn(u64) -> u64) = if board.white_to_move {
        (|b| (b << 7) & NOT_H_FILE, |b| (b << 9) & NOT_A_FILE)
    } else {
        (|b| (b >> 9) & NOT_H_FILE, |b| (b >> 7) & NOT_A_FILE)
    };

    // Left captures
    let mut left_caps = left_shift(pawns) & enemy_occ;
    let mut left_promos = left_caps & promo_rank;
    left_caps &= !promo_rank;

    while left_promos != 0 {
        let to = left_promos.trailing_zeros() as u8;
        let from = if board.white_to_move { to - 7 } else { to + 9 };
        push_promotions(moves, curr_move_index, from, to, true);
        left_promos &= left_promos - 1;
    }
    while left_caps != 0 {
        let to = left_caps.trailing_zeros() as u8;
        let from = if board.white_to_move { to - 7 } else { to + 9 };
        moves[*curr_move_index] = Move::new_with_flags(from, to, Move::CAPTURE);
        *curr_move_index += 1;
        left_caps &= left_caps - 1;
    }

    // Right captures
    let mut right_caps = right_shift(pawns) & enemy_occ;
    let mut right_promos = right_caps & promo_rank;
    right_caps &= !promo_rank;

    while right_promos != 0 {
        let to = right_promos.trailing_zeros() as u8;
        let from = if board.white_to_move { to - 9 } else { to + 7 };
        push_promotions(moves, curr_move_index, from, to, true);
        right_promos &= right_promos - 1;
    }
    while right_caps != 0 {
        let to = right_caps.trailing_zeros() as u8;
        let from = if board.white_to_move { to - 9 } else { to + 7 };
        moves[*curr_move_index] = Move::new_with_flags(from, to, Move::CAPTURE);
        *curr_move_index += 1;
        right_caps &= right_caps - 1;
    }

    // En passant
    if let Some(ep_sq) = board.en_passant_square {
        let ep_bit = 1u64 << ep_sq;
        let left_ep = left_shift(pawns) & ep_bit;
        if left_ep != 0 {
            let from = if board.white_to_move { ep_sq - 7 } else { ep_sq + 9 };
            moves[*curr_move_index] = Move::new_with_flags(from, ep_sq, Move::EN_PASSANT);
            *curr_move_index += 1;
        }
        let right_ep = right_shift(pawns) & ep_bit;
        if right_ep != 0 {
            let from = if board.white_to_move { ep_sq - 9 } else { ep_sq + 7 };
            moves[*curr_move_index] = Move::new_with_flags(from, ep_sq, Move::EN_PASSANT);
            *curr_move_index += 1;
        }
    }
}

fn generate_knight_captures(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let enemy_occ = board.pieces[((us ^ 8) | Piece::ALL) as usize];
    let mut knights = board.pieces[(us | Piece::KNIGHT) as usize];

    while knights != 0 {
        let from = knights.trailing_zeros() as u8;

        // MAGIC TRICK: We only intersect with enemy pieces. 
        // This instantly deletes empty squares and friendly fire.
        let mut attacks = KNIGHT_MOVES[from as usize] & enemy_occ;

        while attacks != 0 {
            let to = attacks.trailing_zeros() as u8;
            moves[*curr_move_index] = Move::new_with_flags(from, to, Move::CAPTURE);
            *curr_move_index += 1;
            attacks &= attacks - 1;
        }
        knights &= knights - 1;
    }
}

fn generate_king_captures(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let enemy = us ^ 8;
    let enemy_occ = board.pieces[(enemy | Piece::ALL) as usize];

    let king_bb = board.pieces[(us | Piece::KING) as usize];
    if king_bb == 0 { return; }
    let from = king_bb.trailing_zeros() as u8;

    // Same magic trick: only intersect with enemy pieces
    let mut attacks = KING_MOVES[from as usize] & enemy_occ;

    while attacks != 0 {
        let to = attacks.trailing_zeros() as u8;
        moves[*curr_move_index] = Move::new_with_flags(from, to, Move::CAPTURE);
        *curr_move_index += 1;
        attacks &= attacks - 1;
    }
    // Notice: Castling logic is COMPLETELY DELETED (you can't capture by castling)
}

fn generate_bishop_captures(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let enemy = us ^ 8;
    let enemy_occ = board.pieces[(enemy | Piece::ALL) as usize];
    let total_occ = board.pieces[0];
    let mut bishops = board.pieces[(us | Piece::BISHOP) as usize];

    while bishops != 0 {
        let from = bishops.trailing_zeros() as u8;

        // Only intersect with enemy pieces for captures
        let mut attacks = get_bishop_attacks(from, total_occ) & enemy_occ;

        while attacks != 0 {
            let to = attacks.trailing_zeros() as u8;
            moves[*curr_move_index] = Move::new_with_flags(from, to, Move::CAPTURE);
            *curr_move_index += 1;
            attacks &= attacks - 1;
        }

        bishops &= bishops - 1;
    }
}

fn generate_rook_captures(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let enemy = us ^ 8;
    let enemy_occ = board.pieces[(enemy | Piece::ALL) as usize];
    let total_occ = board.pieces[0];
    let mut rooks = board.pieces[(us | Piece::ROOK) as usize];

    while rooks != 0 {
        let from = rooks.trailing_zeros() as u8;

        // Only intersect with enemy pieces for captures
        let mut attacks = get_rook_attacks(from, total_occ) & enemy_occ;

        while attacks != 0 {
            let to = attacks.trailing_zeros() as u8;
            moves[*curr_move_index] = Move::new_with_flags(from, to, Move::CAPTURE);
            *curr_move_index += 1;
            attacks &= attacks - 1;
        }

        rooks &= rooks - 1;
    }
}

fn generate_queen_captures(board: &Board, moves: &mut [Move], curr_move_index: &mut usize) {
    let us = if board.white_to_move { Piece::WHITE } else { Piece::BLACK };
    let enemy = us ^ 8;
    let enemy_occ = board.pieces[(enemy | Piece::ALL) as usize];
    let total_occ = board.pieces[0];
    let mut queens = board.pieces[(us | Piece::QUEEN) as usize];

    while queens != 0 {
        let from = queens.trailing_zeros() as u8;

        // Only intersect with enemy pieces for captures (combination of bishop and rook attacks)
        let mut attacks = get_queen_attacks(from, total_occ) & enemy_occ;

        while attacks != 0 {
            let to = attacks.trailing_zeros() as u8;
            moves[*curr_move_index] = Move::new_with_flags(from, to, Move::CAPTURE);
            *curr_move_index += 1;
            attacks &= attacks - 1;
        }

        queens &= queens - 1;
    }
}

pub const KNIGHT_MOVES: [u64; 64] = {
    let mut table = [0u64; 64];
    let mut sq = 0;
    while sq < 64 {
        let mut moves = 0u64;
        let start_file = (sq % 8) as i8;

        // The 8 possible bit-shift offsets for a Knight
        // Positive = Up/Right, Negative = Down/Left
        let offsets: [i8; 8] = [17, 15, 10, 6, -17, -15, -10, -6];

        let mut i = 0;
        while i < 8 {
            let offset = offsets[i];
            let target_sq = sq as i8 + offset;

            // 1. Check if the square is even on the 0-63 board
            if target_sq >= 0 && target_sq < 64 {
                let target_file = (target_sq % 8) as i8;

                // A knight move can never change more than 2 files.
                // If it changed 7 files (e.g. from A to H), it wrapped.
                if (target_file - start_file).abs() <= 2 {
                    moves |= 1u64 << target_sq as u8;
                }
            }
            i += 1;
        }

        table[sq as usize] = moves;
        sq += 1;
    }
    table
};

pub const KING_MOVES: [u64; 64] = {
    let mut table = [0u64; 64];
    let mut sq = 0;
    while sq < 64 {
        let mut attacks = 0u64;
        let start_file = (sq % 8) as i8;
        let offsets: [i8; 8] = [8, -8, 1, -1, 7, 9, -7, -9];

        let mut i = 0;
        while i < 8 {
            let target_sq = sq as i8 + offsets[i];
            if target_sq >= 0 && target_sq < 64 {
                let target_file = (target_sq % 8) as i8;
                // King only moves 1 file away max
                if (target_file - start_file).abs() <= 1 {
                    attacks |= 1u64 << target_sq as u8;
                }
            }
            i += 1;
        }
        table[sq as usize] = attacks;
        sq += 1;
    }
    table
};

// 0xFE = 11111110 (A-file is 0)
pub const NOT_A_FILE: u64 = 0xfefefefefefefefe;

// 0x7F = 01111111 (H-file is 0)
pub const NOT_H_FILE: u64 = 0x7f7f7f7f7f7f7f7f;

