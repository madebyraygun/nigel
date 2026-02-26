import typer

app = typer.Typer(help="Bluepeak Bookkeeper — cash-basis bookkeeping CLI.")


@app.command()
def init():
    """Initialize the bookkeeper database and seed categories."""
    typer.echo("Not yet implemented.")


if __name__ == "__main__":
    app()
