use eframe::egui;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::thread;

const STARTING_BOARD: [char; 64] = [
    '♜', '♞', '♝', '♛', '♚', '♝', '♞', '♜',
    '♟', '♟', '♟', '♟', '♟', '♟', '♟', '♟',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
    '♙', '♙', '♙', '♙', '♙', '♙', '♙', '♙',
    '♖', '♘', '♗', '♕', '♔', '♗', '♘', '♖',
];

#[derive(PartialEq)]
enum PlayerType { Human, Engine }

struct EngineData {
    stdin: std::process::ChildStdin,
    receiver: Receiver<String>,
    log: Vec<String>,
}

struct ChessGui {
    // Player Configurations
    white_type: PlayerType,
    black_type: PlayerType,
    white_path: String,
    black_path: String,

    // Running Engines
    white_engine: Option<EngineData>,
    black_engine: Option<EngineData>,

    // Game State
    board: [char; 64],
    selected_square: Option<usize>,
    move_history: Vec<String>,
    white_to_move: bool,
}

impl Default for ChessGui {
    fn default() -> Self {
        Self {
            white_type: PlayerType::Human,
            black_type: PlayerType::Engine,
            white_path: "./target/debug/chess_engine.exe".to_owned(),
            black_path: "./target/debug/chess_engine.exe".to_owned(),
            white_engine: None,
            black_engine: None,
            board: STARTING_BOARD,
            selected_square: None,
            move_history: Vec::new(),
            white_to_move: true,
        }
    }
}

impl ChessGui {
    /// Spawns an engine and returns its data wrapper
    fn spawn_engine(path: &str) -> Option<EngineData> {
        let spawn_result = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn();

        let mut child = match spawn_result {
            Ok(child) => child,
            Err(e) => {
                eprintln!("CRITICAL ERROR: Could not start engine at [{}]. System error: {}", path, e);
                return None;
            }
        };

        let stdout = child.stdout.take().expect("Failed to open stdout");
        let mut stdin = child.stdin.take().expect("Failed to open stdin");

        let (tx, rx) = channel();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(msg) = line {
                    if tx.send(msg).is_err() { break; }
                }
            }
        });

        // Initialize UCI mode
        let _ = stdin.write_all(b"uci\n");
        let _ = stdin.write_all(b"isready\n");

        Some(EngineData {
            stdin,
            receiver: rx,
            log: vec!["Engine connected and uci sent.".to_string()],
        })
    }

    /// Sends a command to a specific engine
    fn send_to_engine(engine: &mut Option<EngineData>, cmd: &str) {
        if let Some(eng) = engine {
            let _ = writeln!(eng.stdin, "{}", cmd); // writeln! adds the \n automatically
            let _ = eng.stdin.flush(); // Force the bytes out of the buffer and into the engine
            eng.log.push(format!("> {}", cmd));
        }
    }

    /// Triggers the active engine to calculate a move if it's their turn
    fn trigger_active_engine(&mut self) {
        let is_white = self.white_to_move;
        let active_type = if is_white { &self.white_type } else { &self.black_type };
        let active_engine = if is_white { &mut self.white_engine } else { &mut self.black_engine };

        if *active_type == PlayerType::Engine && active_engine.is_some() {
            let moves = self.move_history.join(" ");
            let position_cmd = format!("position startpos moves {}", moves);

            Self::send_to_engine(active_engine, &position_cmd);
            Self::send_to_engine(active_engine, "go movetime 500"); // 500ms think time
        }
    }

    /// Processes a move (from human or engine), updates board, and passes turn
    fn apply_move(&mut self, uci_move: &str) {
        if uci_move.len() < 4 { return; }

        let bytes = uci_move.as_bytes();
        let from_col = (bytes[0] - b'a') as usize;
        let from_row = 7 - (bytes[1] - b'1') as usize;
        let to_col = (bytes[2] - b'a') as usize;
        let to_row = 7 - (bytes[3] - b'1') as usize;

        let from_idx = from_row * 8 + from_col;
        let to_idx = to_row * 8 + to_col;

        // Update visual board
        self.board[to_idx] = self.board[from_idx];
        self.board[from_idx] = ' ';

        self.move_history.push(uci_move.to_string());
        self.selected_square = None;
        self.white_to_move = !self.white_to_move; // Toggle turn

        // Trigger the next player if it's an engine
        self.trigger_active_engine();
    }

    fn index_to_algebraic(index: usize) -> String {
        let file = (b'a' + (index % 8) as u8) as char;
        let rank = (b'8' - (index / 8) as u8) as char;
        format!("{}{}", file, rank)
    }

    /// Reads output from both engines and looks for "bestmove"
    fn process_engine_messages(&mut self, ctx: &egui::Context) {
        let mut repainted = false;
        let mut bestmove_to_apply = None;

        // Helper macro to process an engine's output
        macro_rules! process_rx {
            ($engine:expr) => {
                if let Some(eng) = $engine {
                    while let Ok(msg) = eng.receiver.try_recv() {
                        eng.log.push(msg.clone());
                        repainted = true;

                        // If the engine gives us a move, catch it!
                        if msg.starts_with("bestmove") {
                            let parts: Vec<&str> = msg.split_whitespace().collect();
                            if parts.len() >= 2 {
                                bestmove_to_apply = Some(parts[1].to_string());
                            }
                        }
                    }
                }
            };
        }

        process_rx!(&mut self.white_engine);
        process_rx!(&mut self.black_engine);

        // If an engine returned a move, apply it
        if let Some(m) = bestmove_to_apply {
            self.apply_move(&m);
        }

        if repainted {
            ctx.request_repaint();
        }
    }

    fn render_side_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("engine_panel").exact_width(350.0).show(ctx, |ui| {
            ui.heading("Game Settings");
            ui.label(if self.white_to_move { "Turn: WHITE" } else { "Turn: BLACK" });
            ui.separator();

            // White Player Setup
            ui.heading("White Player");
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.white_type, PlayerType::Human, "Human");
                ui.radio_value(&mut self.white_type, PlayerType::Engine, "Engine");
            });
            if self.white_type == PlayerType::Engine {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.white_path);
                    if ui.button("Start").clicked() {
                        self.white_engine = Self::spawn_engine(&self.white_path);
                        if self.white_to_move { self.trigger_active_engine(); }
                    }
                });
            }

            ui.add_space(10.0);

            // Black Player Setup
            ui.heading("Black Player");
            ui.horizontal(|ui| {
                ui.radio_value(&mut self.black_type, PlayerType::Human, "Human");
                ui.radio_value(&mut self.black_type, PlayerType::Engine, "Engine");
            });
            if self.black_type == PlayerType::Engine {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.black_path);
                    if ui.button("Start").clicked() {
                        self.black_engine = Self::spawn_engine(&self.black_path);
                        if !self.white_to_move { self.trigger_active_engine(); }
                    }
                });
            }

            ui.separator();
            ui.heading("Logs");
            egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                ui.label("--- White Engine ---");
                if let Some(eng) = &self.white_engine {
                    for log in eng.log.iter().rev().take(5).rev() { ui.label(log); }
                }
                ui.label("\n--- Black Engine ---");
                if let Some(eng) = &self.black_engine {
                    for log in eng.log.iter().rev().take(5).rev() { ui.label(log); }
                }
            });
        });
    }

    fn draw_board_and_pieces(&self, painter: &egui::Painter, rect: egui::Rect, square_size: f32) {
        for row in 0..8 {
            for col in 0..8 {
                let index = row * 8 + col;
                let is_light = (row + col) % 2 == 0;

                let mut color = if is_light { egui::Color32::from_rgb(240, 217, 181) }
                else { egui::Color32::from_rgb(181, 136, 99) };

                if self.selected_square == Some(index) { color = egui::Color32::from_rgb(173, 216, 230); }

                let top_left = rect.min + egui::vec2(col as f32 * square_size, row as f32 * square_size);
                let square_rect = egui::Rect::from_min_size(top_left, egui::vec2(square_size, square_size));

                painter.rect_filled(square_rect, 0.0, color);

                let piece = self.board[index];
                if piece != ' ' {
                    let text_color = if "♚♛♜♝♞♟".contains(piece) { egui::Color32::BLACK } else { egui::Color32::WHITE };
                    painter.text(square_rect.center(), egui::Align2::CENTER_CENTER, piece.to_string(), egui::FontId::proportional(square_size * 0.7), text_color);
                }
            }
        }
    }

    fn handle_board_clicks(&mut self, response: &egui::Response, rect: egui::Rect, square_size: f32) {
        // Block human clicks if it's currently an engine's turn!
        let is_human_turn = (self.white_to_move && self.white_type == PlayerType::Human) ||
            (!self.white_to_move && self.black_type == PlayerType::Human);

        if !is_human_turn { return; }

        if response.clicked() {
            if let Some(mouse_pos) = response.interact_pointer_pos() {
                let col = ((mouse_pos.x - rect.min.x) / square_size) as usize;
                let row = ((mouse_pos.y - rect.min.y) / square_size) as usize;

                if col < 8 && row < 8 {
                    let clicked_index = row * 8 + col;

                    if let Some(start_idx) = self.selected_square {
                        if start_idx != clicked_index {
                            let uci_move = format!("{}{}", Self::index_to_algebraic(start_idx), Self::index_to_algebraic(clicked_index));
                            // Use our new unified move applier!
                            self.apply_move(&uci_move);
                        } else {
                            self.selected_square = None;
                        }
                    } else if self.board[clicked_index] != ' ' {
                        // Optional: Ensure the human only clicks their own color pieces here later
                        self.selected_square = Some(clicked_index);
                    }
                }
            }
        }
    }
}

impl eframe::App for ChessGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_engine_messages(ctx);
        self.render_side_panel(ctx);

        // The CentralPanel automatically takes up whatever space is left
        // after the SidePanel is drawn.
        egui::CentralPanel::default().show(ctx, |ui| {

            // 1. Get the available space in the panel
            let available = ui.available_size();

            // 2. Make the board a perfect square by taking the smallest dimension.
            // Multiplying by 0.95 gives us a nice 5% padding so it doesn't touch the window edges.
            let board_size = available.min_elem() * 0.95;

            // 3. Calculate offsets to perfectly center the board
            let x_offset = (available.x - board_size) / 2.0;
            let y_offset = (available.y - board_size) / 2.0;

            // 4. Apply the offsets and draw
            ui.add_space(y_offset); // Push down
            ui.horizontal(|ui| {
                ui.add_space(x_offset); // Push right

                let (response, painter) = ui.allocate_painter(
                    egui::vec2(board_size, board_size),
                    egui::Sense::click()
                );

                let rect = response.rect;
                let square_size = rect.width() / 8.0;

                self.draw_board_and_pieces(&painter, rect, square_size);
                self.handle_board_clicks(&response, rect, square_size);
            });
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    eframe::run_native("Rust Chess Arena", eframe::NativeOptions::default(), Box::new(|_cc| Ok(Box::<ChessGui>::default())))
}