# Sound Mod · Rust Neon Edition

[English](README.md) | **Português**

![Sound Mod](docs/screenshot-pt.png)

**Soundboard para Windows que toca sons direto no seu microfone**, refeito em Rust com uma interface neon em grafite escuro e verde fluorescente. Aperte uma tecla (ou clique num botão) e o som sai no canal do microfone, para Discord, jogos, chamadas e streams, enquanto você continua falando normalmente.

Também grava os **últimos segundos do áudio do PC** (padrão 5 s) para você cortar e reproduzir no microfone.

## Recursos

- **Hotkeys globais**: F1, F2, F3... tocam sons diferentes mesmo com o app em segundo plano **ou minimizado**. Aperte uma vez para tocar e de novo para parar. As teclas podem ser trocadas (Esc cancela) ou removidas, e combinações como `Ctrl+1` funcionam. O gancho só observa: a tecla continua chegando ao jogo.
- **Mic + sons juntos**: mistura seu microfone real com os sons, sem trocar de dispositivo.
- **Vários formatos, nada para instalar**: mp3, wav, ogg, flac, m4a/aac, opus, aiff... decodificados nativamente. Para o resto (ex.: wma), coloque um `ffmpeg.exe` ao lado do programa.
- **Controle por som**: ▶ tocar, ⏸ pausar/continuar, ■ parar, barra de progresso arrastável e volume individual ao vivo.
- **Um som por vez**: tocar um som interrompe o atual.
- **Vários arquivos de uma vez**: o **Adicionar som** aceita vários arquivos; eles ocupam primeiro os slots vazios.
- **Efeitos de voz**: Grave, Agudo, Robô, Ogro, Autotune, Eco, Rádio e Alien, aplicados ao vivo no microfone. Ligue **Ouvir minha voz** para se escutar.
- **Clipe do áudio do PC**: uma tecla salva os últimos N segundos em `clips/last_clip.wav`; outra toca. Arraste as alças neon na forma de onda para **escolher o trecho**, e **Salvar como som** guarda esse trecho como um som novo (na pasta `sounds/`).
- **Monitor nos fones**: você ouve os sons enquanto eles saem no microfone.
- **Medidores de nível** do microfone e da saída, e indicador **Ao vivo** pulsando.
- **Português e inglês**: segue o idioma do Windows e pode ser trocado a qualquer momento.
- **Um único `.exe`**, sem runtime nem Python, abre na hora.

## Como funciona

O Windows não permite injetar áudio num microfone físico, então o app usa um **cabo de áudio virtual** (VB-CABLE):

```
Microfone real ──┐
                 ├─► Sound Mod ─► CABLE Input ═► CABLE Output ─► Discord/jogo (como microfone)
Sons (F1, F2...) ┘      └─► seus fones (monitor)
```

## Instalação

> **Ainda sem release.** O projeto está sendo refinado antes da v1.0; por enquanto, compile a partir do código como descrito abaixo.

1. Instale o [VB-CABLE](https://vb-audio.com/Cable/) (gratuito) e reinicie o PC.
2. Compile o programa (abaixo) e abra `dist\SoundMod\SoundMod.exe`. Deixe-o numa pasta própria: configurações, clipes e sons salvos ficam ao lado dele.

### Compilar

Requer Windows 10/11 e o [rustup](https://rustup.rs). **Não precisa do Visual Studio.**

```powershell
powershell -ExecutionPolicy Bypass -File build.ps1
```

Gera `dist\SoundMod\SoundMod.exe`. Recompilar só troca o `.exe`: seu `config.json`, `clips\` e `sounds\` continuam lá. Outros modos: `build.ps1 -Run` (compila em debug e abre), `build.ps1 -Test` (testes), `build.ps1 -Test -Hardware` (também toca um tom curto e baixo no VB-CABLE e grava de volta para conferir todo o caminho do áudio; nada sai nos seus alto-falantes).

O projeto usa o toolchain GNU do Rust (fixado no `rust-toolchain.toml`) com o linker `rust-lld` do LLVM, ambos instalados pelo rustup. O `build.ps1` também prepara o `dlltool` que as crates do Windows exigem (é o `llvm-ar` do componente `llvm-tools`).

## Como usar

1. **Microfone real**: seu microfone físico. **Saída → cabo virtual**: `CABLE Input`. **Ouvir sons em**: seus fones (não escolha o cabo aqui). O app escolhe isso sozinho na primeira vez.
2. No Discord ou no jogo, escolha **CABLE Output** como microfone. No Discord, desligue Supressão de ruído, Cancelamento de eco e Controle automático de ganho, que costumam cortar os efeitos sonoros.
3. Escolha os arquivos dos slots. Cada botão de tecla é dividido: **▶ F1** toca e, apertado de novo, para; **✎** troca a tecla; **⌫** remove a tecla. Só um som toca por vez.
4. **F5** salva os últimos N segundos do áudio do PC e mostra a forma de onda; arraste as alças verdes para escolher o trecho. **F6** toca esse trecho. **F12** para tudo. **Salvar como som** pede um nome e adiciona o trecho escolhido aos seus sons.
5. **Efeito de voz**: escolha um efeito no painel lateral. Ele muda seu microfone ao vivo; **Normal** desliga. Marque **Ouvir minha voz** para se escutar.

Trocas de dispositivo valem ao clicar em **Aplicar / Reiniciar**; interruptores e volumes valem na hora.

## Vindo da versão em Python

Esta é uma reescrita em Rust do [Sound Mod (Python)](https://github.com/renanzm/sound-mod). O `config.json` tem o mesmo formato: copie seu `config.json` e a pasta `sounds\` para o lado do novo `.exe` e seus slots, teclas e dispositivos continuam. Diferenças: clipes e sons salvos ficam em WAV (sem perda, sem precisar de codificador) em vez de MP3, e o `last_clip.mp3` antigo não é usado.

## Documentação

Manual de arquitetura e desenvolvimento: [docs/ARQUITETURA.md](docs/ARQUITETURA.md).

## Limitações conhecidas

- Só Windows (WASAPI, gancho de teclado de baixo nível, estilo de janela do DWM).
- Requer o VB-CABLE (ou outro cabo virtual).
- Para capturar teclas com um jogo rodando como administrador em foco, rode o Sound Mod como administrador.
- O loopback do WASAPI não entrega nada enquanto o PC está em silêncio, então o clipe pode sair vazio ("Nada gravado ainda").

## Licença

[MIT](LICENSE)
