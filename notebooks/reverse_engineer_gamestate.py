import marimo

__generated_with = "0.24.0"
app = marimo.App(width="medium")


@app.cell
def _():
    import pathlib
    import os
    import marimo as mo

    return mo, pathlib


@app.cell
def _(pathlib):
    POLYTOPIA_PATH = pathlib.Path('/home/eaglehawkinator/.config/unity3d/Midjiwan/Polytopia/47dd190b-a946-4efb-b947-3c54deb5a7b6/Data/Singleplayer')
    GAME_ID = 'c14fc5be-d04a-4233-93fb-f63627ce7e35.state'
    return GAME_ID, POLYTOPIA_PATH


@app.cell
def _(GAME_ID, POLYTOPIA_PATH):
    game_path = POLYTOPIA_PATH / GAME_ID
    return (game_path,)


@app.function
def hexdump(path, length=None):
    with open(path, 'rb') as f:
        num_lines = None if length is None else length * 16
        data = f.read(num_lines)

    data_string = ''

    for i in range(0, len(data), 16):
        chunk = data[i:i+16]
        hex_part = ' '.join(f'{b:02x}' for b in chunk)
        ascii_part = ''.join(chr(b) if 32 <= b <= 126 else '.' for b in chunk)
        data_string += f"{i:08x}: {hex_part:<47} {ascii_part}\n"

    return data_string


@app.cell
def _(game_path):
    print(hexdump(game_path, 5))
    return


@app.cell
def _():
    hex_strings = []
    move_descriptions = []
    return hex_strings, move_descriptions


@app.cell(hide_code=True)
def _(mo):
    mo.md(r"""
    Now, we can play through a game and capture the gamestate after every move.

    The gamestate is only written to disk on a save (exiting to the main menu), so be sure to do that in between moves.
    """)
    return


@app.cell
def _(game_path, hex_strings, move_descriptions):
    def button_press(value):
        hex_strings.append(hexdump(game_path))
        move_descriptions.append(input("What move did you just make? "))
    

    return (button_press,)


@app.cell
def _(button_press, mo):
    button = mo.ui.button(
        label = "Capture gamestate",
        on_click = button_press
    )
    button
    return


@app.cell
def _(hex_strings, move_descriptions):
    len(hex_strings), len(move_descriptions)
    return


@app.cell
def _():
    return


if __name__ == "__main__":
    app.run()
