use eframe::egui;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use chess_engine::{Board, Move, Piece, generate_all_moves, is_square_attacked};

#[derive(PartialEq, Clone, Copy)]
enum PlayerType { Human, Engine }

struct EngineData {
    stdin: std::process::ChildStdin,
    receiver: Receiver<String>,
    log: Vec<String>,
}

struct ChessGui {
    white_type: PlayerType,
    black_type: PlayerType,
    white_path: String,
    black_path: String,

    white_engine: Option<EngineData>,
    black_engine: Option<EngineData>,

    board_state: Board,
    selected_square: Option<usize>,
    move_history: Vec<String>,
    status_message: String,
}

impl Default for ChessGui {
    fn default() -> Self {
        Self {
            white_type: PlayerType::Human,
            black_type: PlayerType::Engine,
            white_path: "./target/release/chess_engine.exe".to_owned(),
            black_path: "./target/release/chess_engine.exe".to_owned(),
            white_engine: None,
            black_engine: None,
            board_state: Board::starting_position(),
            selected_square: None,
            move_history: Vec::new(),
            status_message: "Game Start".to_string(),
        }
    }
}

impl ChessGui {
    fn get_piece_char(&self, index: usize) -> char {
        let bit = 1u64 << index;
        let p = &self.board_state.pieces;

        if (p[(Piece::WHITE | Piece::PAWN) as usize] & bit) != 0 { return '♙'; }
        if (p[(Piece::WHITE | Piece::KNIGHT) as usize] & bit) != 0 { return '♘'; }
        if (p[(Piece::WHITE | Piece::BISHOP) as usize] & bit) != 0 { return '♗'; }
        if (p[(Piece::WHITE | Piece::ROOK) as usize] & bit) != 0 { return '♖'; }
        if (p[(Piece::WHITE | Piece::QUEEN) as usize] & bit) != 0 { return '♕'; }
        if (p[(Piece::WHITE | Piece::KING) as usize] & bit) != 0 { return '♔'; }

        if (p[(Piece::BLACK | Piece::PAWN) as usize] & bit) != 0 { return '♟'; }
        if (p[(Piece::BLACK | Piece::KNIGHT) as usize] & bit) != 0 { return '♞'; }
        if (p[(Piece::BLACK | Piece::BISHOP) as usize] & bit) != 0 { return '♝'; }
        if (p[(Piece::BLACK | Piece::ROOK) as usize] & bit) != 0 { return '♜'; }
        if (p[(Piece::BLACK | Piece::QUEEN) as usize] & bit) != 0 { return '♛'; }
        if (p[(Piece::BLACK | Piece::KING) as usize] & bit) != 0 { return '♚'; }

        ' '
    }

    fn is_black_piece(&self, index: usize) -> bool {
        let bit = 1u64 << index;
        (self.board_state.pieces[15] & bit) != 0
    }

    fn is_white_piece(&self, index: usize) -> bool {
        let bit = 1u64 << index;
        (self.board_state.pieces[7] & bit) != 0
    }

    fn check_game_status(&mut self) {
        let mut moves = [Move(0); 256];
        let count = generate_all_moves(&self.board_state, &mut moves);

        let mut legal_move_found = false;
        let us = if self.board_state.white_to_move { Piece::WHITE } else { Piece::BLACK };
        let enemy = us ^ 8;

        for i in 0..count {
            let mut temp_board = self.board_state;
            temp_board.make_move(moves[i]);
            let king_bit = temp_board.pieces[(us | Piece::KING) as usize];
            if king_bit != 0 {
                let king_sq = king_bit.trailing_zeros() as u8;
                if !is_square_attacked(&temp_board, king_sq, enemy) {
                    legal_move_found = true;
                    break;
                }
            }
        }

        if !legal_move_found {
            let king_bit = self.board_state.pieces[(us | Piece::KING) as usize];
            let king_sq = king_bit.trailing_zeros() as u8;
            if is_square_attacked(&self.board_state, king_sq, enemy) {
                self.status_message = format!("CHECKMATE! {} wins.", if self.board_state.white_to_move { "Black" } else { "White" });
            } else {
                self.status_message = "STALEMATE!".to_string();
            }
        } else {
            self.status_message = format!("{}'s Turn", if self.board_state.white_to_move { "White" } else { "Black" });
        }
    }

    fn apply_move(&mut self, uci_move: &str) {
        if let Some(m) = self.board_state.parse_uci_to_move(uci_move) {
            self.board_state.make_move(m);
            self.move_history.push(uci_move.to_string());
            self.selected_square = None;
            self.check_game_status();
            // Crucial: Pass control to the next player/engine
            self.trigger_active_engine();
        }
    }

    fn spawn_engine(path: &str, ctx: egui::Context) -> Option<EngineData> {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .ok()?;

        let stdout = child.stdout.take()?;
        let mut stdin = child.stdin.take()?;
        let (tx, rx) = channel();

        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(msg) = line {
                    if tx.send(msg).is_err() { break; }

                    ctx.request_repaint();
                }
            }
        });

        let _ = writeln!(stdin, "uci");
        let _ = writeln!(stdin, "isready");
        let _ = stdin.flush();

        Some(EngineData { stdin, receiver: rx, log: vec!["Engine connected.".into()] })
    }

    fn trigger_active_engine(&mut self) {
        let is_white = self.board_state.white_to_move;

        // Choose which engine to command based on whose turn it is
        if is_white && self.white_type == PlayerType::Engine {
            if let Some(eng) = &mut self.white_engine {
                let moves = self.move_history.join(" ");
                let _ = writeln!(eng.stdin, "position startpos moves {}", moves);
                let _ = writeln!(eng.stdin, "go movetime 1000"); // 1 second think time
                let _ = eng.stdin.flush();
            }
        } else if !is_white && self.black_type == PlayerType::Engine {
            if let Some(eng) = &mut self.black_engine {
                let moves = self.move_history.join(" ");
                let _ = writeln!(eng.stdin, "position startpos moves {}", moves);
                let _ = writeln!(eng.stdin, "go movetime 1000");
                let _ = eng.stdin.flush();
            }
        }
    }

    fn process_engine_messages(&mut self, ctx: &egui::Context) {
        let mut bestmove = None;
        let mut needs_repaint = false;
        if let Some(eng) = &mut self.white_engine {
            while let Ok(msg) = eng.receiver.try_recv() {
                eng.log.push(msg.clone());
                needs_repaint = true;
                if msg.starts_with("bestmove") {
                    bestmove = msg.split_whitespace().nth(1).map(|s| s.to_string());
                }
            }
        }
        if let Some(eng) = &mut self.black_engine {
            while let Ok(msg) = eng.receiver.try_recv() {
                eng.log.push(msg.clone());
                needs_repaint = true;
                if msg.starts_with("bestmove") {
                    bestmove = msg.split_whitespace().nth(1).map(|s| s.to_string());
                }
            }
        }
        if let Some(m) = bestmove {
            self.apply_move(&m);
            needs_repaint = true;
        }

        if needs_repaint {
            ctx.request_repaint();
        }
    }
}

impl eframe::App for ChessGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_engine_messages(ctx);

        egui::SidePanel::right("panel").exact_width(320.0).show(ctx, |ui| {
            ui.add_space(10.0);
            ui.heading("Game Status");
            ui.colored_label(egui::Color32::LIGHT_BLUE, &self.status_message);
            ui.separator();

            // White Player Group
            ui.group(|ui| {
                ui.heading("White Player");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.white_type, PlayerType::Human, "Human");
                    ui.radio_value(&mut self.white_type, PlayerType::Engine, "Engine");
                });
                if self.white_type == PlayerType::Engine {
                    egui::Grid::new("white_grid").num_columns(2).show(ui, |ui| {
                        ui.label("Path:");
                        ui.text_edit_singleline(&mut self.white_path);
                        ui.end_row();
                        if ui.button("Connect White").clicked() {
                            self.white_engine = ChessGui::spawn_engine(&self.white_path, ctx.clone());
                            // Auto-trigger if it's White's turn
                            if self.board_state.white_to_move { self.trigger_active_engine(); }
                        }
                        if self.white_engine.is_some() { ui.colored_label(egui::Color32::GREEN, "Active"); }
                    });
                }
            });

            ui.add_space(10.0);

            // Black Player Group
            ui.group(|ui| {
                ui.heading("Black Player");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.black_type, PlayerType::Human, "Human");
                    ui.radio_value(&mut self.black_type, PlayerType::Engine, "Engine");
                });
                if self.black_type == PlayerType::Engine {
                    egui::Grid::new("black_grid").num_columns(2).show(ui, |ui| {
                        ui.label("Path:");
                        ui.text_edit_singleline(&mut self.black_path);
                        ui.end_row();
                        if ui.button("Connect Black").clicked() {
                            self.black_engine = ChessGui::spawn_engine(&self.black_path, ctx.clone());
                            // Auto-trigger if it's Black's turn
                            if !self.board_state.white_to_move { self.trigger_active_engine(); }
                        }
                        if self.black_engine.is_some() { ui.colored_label(egui::Color32::GREEN, "Active"); }
                    });
                }
            });

            ui.add_space(20.0);
            ui.heading("Engine Logs");
            ui.label("⚪ White");
            // Fixed id_salt warnings here
            egui::ScrollArea::vertical().id_salt("w_log").max_height(120.0).stick_to_bottom(true).show(ui, |ui| {
                if let Some(e) = &self.white_engine { for l in &e.log { ui.add(egui::Label::new(egui::RichText::new(l).monospace().size(10.0))); } }
            });
            ui.add_space(5.0);
            ui.label("⚫ Black");
            egui::ScrollArea::vertical().id_salt("b_log").max_height(120.0).stick_to_bottom(true).show(ui, |ui| {
                if let Some(e) = &self.black_engine { for l in &e.log { ui.add(egui::Label::new(egui::RichText::new(l).monospace().size(10.0))); } }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let size = ui.available_size().min_elem() * 0.95;
            let (response, painter) = ui.allocate_painter(egui::vec2(size, size), egui::Sense::click());
            let rect = response.rect;
            let square_size = rect.width() / 8.0;

            // Draw Board
            for row_gui in 0..8 {
                for col in 0..8 {
                    let index = (7 - row_gui) * 8 + col;
                    let is_light = (row_gui + col) % 2 == 0;
                    let mut color = if is_light { egui::Color32::from_rgb(240, 217, 181) } else { egui::Color32::from_rgb(181, 136, 99) };
                    if self.selected_square == Some(index) { color = egui::Color32::from_rgb(173, 216, 230); }

                    let sq_rect = egui::Rect::from_min_size(rect.min + egui::vec2(col as f32 * square_size, row_gui as f32 * square_size), egui::vec2(square_size, square_size));
                    painter.rect_filled(sq_rect, 0.0, color);

                    let piece = self.get_piece_char(index);
                    if piece != ' ' {
                        let text_color = if self.is_black_piece(index) { egui::Color32::BLACK } else { egui::Color32::WHITE };
                        painter.text(sq_rect.center(), egui::Align2::CENTER_CENTER, piece.to_string(), egui::FontId::proportional(square_size * 0.8), text_color);
                    }
                }
            }

            // Click handling logic (Selection / Move completion)
            if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let col = ((pos.x - rect.min.x) / square_size) as usize;
                    let row_gui = ((pos.y - rect.min.y) / square_size) as usize;
                    let clicked_idx = (7 - row_gui) * 8 + col;

                    let is_white_to_move = self.board_state.white_to_move;
                    let is_human_turn = if is_white_to_move { self.white_type == PlayerType::Human } else { self.black_type == PlayerType::Human };

                    if is_human_turn {
                        if let Some(start_idx) = self.selected_square {
                            let uci = format!("{}{}{}{}",
                                              (b'a' + (start_idx % 8) as u8) as char, (b'1' + (start_idx / 8) as u8) as char,
                                              (b'a' + (clicked_idx % 8) as u8) as char, (b'1' + (clicked_idx / 8) as u8) as char
                            );

                            if self.board_state.parse_uci_to_move(&uci).is_some() {
                                self.apply_move(&uci);
                            } else {
                                let is_own_piece = if is_white_to_move { self.is_white_piece(clicked_idx) } else { self.is_black_piece(clicked_idx) };
                                if is_own_piece { self.selected_square = Some(clicked_idx); }
                                else { self.selected_square = None; }
                            }
                        } else {
                            let is_own_piece = if is_white_to_move { self.is_white_piece(clicked_idx) } else { self.is_black_piece(clicked_idx) };
                            if is_own_piece { self.selected_square = Some(clicked_idx); }
                        }
                    }
                }
            }
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let mut options = eframe::NativeOptions::default();
    options.viewport = egui::ViewportBuilder::default()
        .with_inner_size([1200.0, 800.0]);

    eframe::run_native("Rust Chess Arena", options, Box::new(|_| Ok(Box::<ChessGui>::default())))
}