use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write, BufReader, BufWriter};
use std::sync::{Arc, Mutex};
use std::thread;
use rand::Rng;

// Тип для представления игровой доски
type Board = [[char; 10]; 10];

// Структура для хранения состояния игры
struct GameState {
    player1: Option<TcpStream>,
    player2: Option<TcpStream>,
    board1: Board,
    board2: Board,
    current_turn: usize,
    game_started: bool,
    ships1: usize,
    ships2: usize,
}

impl GameState {
    fn new() -> Self {
        let empty_board = [['.'; 10]; 10];
        GameState {
            player1: None,
            player2: None,
            board1: empty_board,
            board2: empty_board,
            current_turn: 1,
            game_started: false,
            ships1: 10, // 1x4, 2x3, 3x2, 4x1
            ships2: 10,
        }
    }

    fn is_ready(&self) -> bool {
        self.player1.is_some() && self.player2.is_some()
    }
}


fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:7878")?;
    println!("Сервер запущен на 127.0.0.1:7878");

    let game_state = Arc::new(Mutex::new(GameState::new()));

    for stream in listener.incoming() {
        let stream = stream?;
        let game_state = Arc::clone(&game_state);
        thread::spawn(move || {
            handle_client(stream, game_state).unwrap_or_else(|e| eprintln!("Ошибка: {}", e));
        });
    }

    Ok(())
}

fn handle_client(stream: TcpStream, game_state: Arc<Mutex<GameState>>) -> std::io::Result<()> {
    let mut reader = BufReader::new(&stream);
    let mut writer = BufWriter::new(&stream);
    let mut buffer = [0; 1024];

    // Читаем идентификатор игрока
    let bytes_read = reader.read(&mut buffer)?;
    let player_id = String::from_utf8_lossy(&buffer[..bytes_read]).trim().parse::<usize>().unwrap();

    println!("Подключился игрок {}", player_id);

    {
        let mut state = game_state.lock().unwrap();
        if player_id == 1 {
            state.player1 = Some(stream.try_clone()?);
            setup_board(&mut state.board1);
        } else {
            state.player2 = Some(stream.try_clone()?);
            setup_board(&mut state.board2);
        }

        if state.is_ready() && !state.game_started {
            state.game_started = true;
            println!("Оба игрока подключены, начинаем игру!");

            let msg = "Игра началась! Ваш ход.\n";
            if let Some(ref mut player) = state.player1 {
                player.write_all(msg.as_bytes())?;
            }
            if let Some(ref mut player) = state.player2 {
                player.write_all("Игра началась! Ждите своего хода.\n".as_bytes())?;
            }
        }
    }
    loop {
        let mut state = game_state.lock().unwrap();

        if state.current_turn != player_id || !state.game_started {
            drop(state);
            thread::sleep(std::time::Duration::from_millis(100));
            continue;
        }

        // Отправляем игровое поле
        let my_board = if player_id == 1 { &state.board1 } else { &state.board2 };
        let enemy_board = if player_id == 1 { &state.board2 } else { &state.board1 };

        let mut message = String::new();
        message.push_str("\nВаше поле:\n");
        message.push_str(&display_board(my_board, false));
        message.push_str("\nПоле противника:\n");
        message.push_str(&display_board(enemy_board, true));
        message.push_str("\nВаш ход. Введите координаты (например, A5): ");

        writer.write_all(message.as_bytes())?;
        writer.flush()?;

        // Считываем ход
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            return Ok(());
        }

        let input = String::from_utf8_lossy(&buffer[..bytes_read]).trim().to_uppercase();
        let (x, y) = match parse_input(&input) {
            Ok(coords) => coords,
            Err(e) => {
                writer.write_all(format!("Ошибка: {}. Попробуйте ещё раз: ", e).as_bytes())?;
                writer.flush()?;
                continue;
            }
        };

        // Обработка выстрела

        let (result, sunk) = if player_id == 1{
            process_shot(&mut state.board2, x, y)
        } else {
            process_shot(&mut state.board1, x, y)
        };

        // Обновление счётчика кораблей
        if player_id == 1{
            if sunk {
                state.ships2 -= 1;
            }
            else{
                if sunk {
                    state.ships1 -= 1
                }
            }
        }

        //Отправить результат
        let response = if result {
            if sunk {
                "Попадание! Корабль потоплен!\n"
            } else {
                "Попадание!\n"
            }
        } else {
            "Промах!\n"
        };

        writer.write_all(response. as_bytes())?;
        writer.flush()?;

        // Меняем ход

        state.current_turn = if player_id == 1 {2} else {1};

        // Уведомление для другого игрока

        let opponent_msg = format!(
            "Противник выстрелил в {}. {}\nКораблей осталось: {}\nВаш ход.\n",
            input,
            response,
            if player_id == 1 {state.ships1} else {state.ships2});


        if let Some(ref mut player) = if player_id == 1 {&mut state.player2 } else {&mut state.player1}{
            player.write_all(opponent_msg.as_bytes())?
        }

        // Проверка победы

        if(player_id == 1 && state.ships2 == 0) || (player_id == 2 && state.ships1 == 0){
            writer.write_all("Вы победили\n".as_bytes())?;
            if let Some(ref mut player ) = if player_id == 1 {&mut state.player2} else {&mut state.player1}{
                player.write_all("Вы проиграли!\n".as_bytes())?;
            }
            break;
        }

        drop(state);

    }
    Ok(())

}

// Функции для работы с игрой
fn process_shot(board: &mut Board, x: usize, y: usize) -> (bool, bool){
    if board[y][x] == 'S' {
        board[y][x] = 'X';
        let sunk = check_sunk_ship(board, x, y);
        (true, sunk)
    }
    else if board[y][x] == '.'{
        board[y][x] = 'O';
        (false, false)
    }
    else {
        (false, false)
    }
}


fn check_sunk_ship(board: &Board, x: usize, y: usize) -> bool{
    let mut visited_matrix = [[false; 10]; 10];
    let mut stack = vec![(x, y)];
    let mut is_sunk = true;

    while let Some((cx, cy)) = stack.pop() {
        if visited_matrix[cy][cx]{
            continue;
        }

        visited_matrix[cy][cx] = true;

        if board[cy][cx] == 'S' {
            is_sunk = false;
            break;
        }

        if board[cy][cx] == 'X'{
            for(dx, dy) in &[(0,1), (1,0), (0, -1), (-1, 0)]{
                let nx = cx as isize + dx;
                let ny = cy as isize + dy;
                if nx >= 0 && nx < 10 && ny >= 0 && ny < 10{
                    stack.push((nx as usize, ny as usize));
                }
            }
        }

    }
    is_sunk
}
fn parse_input(input: &str) -> std::io::Result<(usize, usize)>{
    if input.len() < 2{
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Неверный формат ввода"));
    }

    let x_char = input.chars().next().unwrap().to_ascii_uppercase();
    let y_str = &input[1..];

    if !('A'..='J').contains(&x_char){
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Буква должна быть от A до J"));
    }

    let x = (x_char as usize) - ('A' as usize);
    let y = y_str.parse::<usize>()
        .map_err( |_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Неверный формат числа"))?;

    if y < 1 || y > 10 {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Число должно быть от 1 до 10"));
    }

    Ok((x, y-1))


}


fn display_board(board: &Board, hide_ships: bool) -> String {
    let mut result = String::new();
    result.push_str("   A B C D E F G H I J\n");

    for (y, row) in board.iter().enumerate() {
        result.push_str(&format!("{:2} ", y + 1));

        for cell in row {
            match cell {
                'S' if hide_ships => result.push_str(". "),
                'S' => result.push_str("S "),
                'X' => result.push_str("X "),
                'O' => result.push_str("O "),
                _ => result.push_str(". "),
            }
        }
        result.push('\n');
    }

    result
}

fn setup_board(board: &mut Board) {
    // Очищаем доску
    *board = [['.'; 10]; 10];

    // Расставляем корабли по правилам (1x4, 2x3, 3x2, 4x1)
    let ships = vec![4, 3, 3, 2, 2, 2, 1, 1, 1, 1];
    let mut rng = rand::rng();

    for &size in &ships {
        loop {
            let horizontal = rng.random_bool(0.5);
            let x = rng.random_range(0..if horizontal { 10 - size } else { 10 });
            let y = rng.random_range(0..if horizontal { 10 } else { 10 - size });

            if can_place_ship(board, x, y, size, horizontal) {
                place_ship(board, x, y, size, horizontal);
                break;
            }
        }
    }
}

fn can_place_ship(board: &Board, x: usize, y: usize, size: usize, horizontal: bool) -> bool {
    let (x_end, y_end) = if horizontal {
        (x + size - 1, y)
    } else {
        (x, y + size - 1)
    };

    // Проверяем границы
    if x_end >= 10 || y_end >= 10 {
        return false;
    }

    // Проверяем соседние клетки
    for i in x.saturating_sub(1)..=x_end.saturating_add(1).min(9) {
        for j in y.saturating_sub(1)..=y_end.saturating_add(1).min(9) {
            if board[j][i] == 'S' {
                return false;
            }
        }
    }
    true
}

fn place_ship(board: &mut Board, x: usize, y: usize, size: usize, horizontal: bool){
    if horizontal {
        for i in x..x +size{
            board[y][i] = 'S';
        }
    }
    else {
        for j in y..y + size{
            board[j][x] = 'S';
        }
    }
}