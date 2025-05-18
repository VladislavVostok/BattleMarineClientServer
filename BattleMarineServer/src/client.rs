use std::io::{Read, Write, BufWriter, BufReader};
use std::net::{TcpStream};
use std::{io, thread};

fn main() -> std::io::Result<()>{
    println!("Добро пожаловать в Морской бой!");

    // Подключение к серверу
    let mut stream = TcpStream::connect("45.139.78.128:7878")?;
    println!("Подключение к серверу 127.0.0.1:7878 - успешно");

    // Запрашиваем идентификатор игрока
    println!("Введите ваш идентификатор (1 или 2):");

    let mut player_id = String::new();
    io::stdin().read_line(&mut player_id)?;

    let player_id = player_id.trim();

    stream.write_all(player_id.as_bytes())?;

    let stream_cln = stream.try_clone()?;

    thread::spawn(move || {

        let mut reader = BufReader::new(stream_cln);
        let mut buffer = [0; 1024];

        loop{
            match reader.read(&mut buffer){
                Ok(0) => {
                    println!("Соединение с сервером разорвано");
                    break;
                }
                Ok(n) => {
                    let msg = String::from_utf8_lossy(&buffer[..n]);
                    println!("{}", msg);
                }
                Err(e) => {
                    eprintln!("Ошибка чтения: {}", e);
                    break;
                }
            }
        }
    });

    let mut writer = BufWriter::new(stream);
    let mut input = String::new();

    loop{
        input.clear();
        io::stdin().read_line(&mut input)?;
        writer.write_all(input.trim().as_bytes())?;
        writer.flush()?;
    }
}
