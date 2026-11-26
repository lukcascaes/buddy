import { currentMonitor, getCurrentWindow, PhysicalPosition } from "@tauri-apps/api/window";

/** Distância da borda da tela, em px lógicos. */
const MARGEM = 12;

/**
 * Coloca a janela no canto inferior direito, logo acima da barra de tarefas.
 * Usa a "workArea" do monitor (a tela MENOS a barra de tarefas), então funciona
 * com a barra em qualquer altura ou lado.
 */
export async function posicionarNoCanto(): Promise<void> {
  const janela = getCurrentWindow();
  const monitor = await currentMonitor();
  if (!monitor) return;

  const tamanho = await janela.outerSize();
  const area = monitor.workArea;
  const margem = MARGEM * monitor.scaleFactor;

  const x = area.position.x + area.size.width - tamanho.width - margem;
  const y = area.position.y + area.size.height - tamanho.height - margem;
  await janela.setPosition(new PhysicalPosition(Math.round(x), Math.round(y)));
}
