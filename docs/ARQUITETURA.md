# Sound Mod (Rust) — Manual de Arquitetura e Desenvolvimento

[English](ARCHITECTURE.md) | **Português**

Soundboard para Windows que toca sons no canal do microfone, com hotkeys globais e gravação dos últimos N segundos do áudio do PC. Reescrita em Rust do Sound Mod em Python, com as mesmas funções e uma interface neon própria.

## 1. Visão geral

```
Microfone real ──┐
                 ├─► [Engine: mixer 48 kHz] ─► CABLE Input ═► CABLE Output ─► Discord/jogo (microfone)
Sons (F1..F4) ───┘          │
                            └─► Fones (monitor, só para você ouvir)

Áudio do PC ─► WASAPI loopback ─► buffer circular (N s) ─► F5 grava WAV ─► F6 toca como som
```

O app depende do VB-CABLE. Sem ele a interface avisa e o mixer não tem para onde enviar.

### Stack

| Função | Crate |
|---|---|
| Interface (modo imediato, OpenGL) | `eframe` / `egui` 0.37 |
| I/O de áudio WASAPI (mic, cabo, fones, loopback) | `cpal` 0.18 |
| Decodificar mp3/wav/ogg/flac/m4a/aac/aiff... | `symphonia` 0.5 (Rust puro) |
| FFT dos efeitos Robô e Autotune | `realfft` |
| Gravar WAV | `hound` |
| Seletor de arquivos | `rfd` |
| Gancho de teclado, idioma, barra de título | `windows-sys` |
| Config | `serde` / `serde_json` |

Tudo é Rust puro: não há código C para compilar.

## 2. Estrutura

```
sound-mod-rs/
├── build.ps1               # compila (release em dist\SoundMod), roda, testa
├── build.rs                # grava ícone + versão no .exe (gera o .res sem rc.exe)
├── rust-toolchain.toml     # stable GNU + llvm-tools
├── .cargo/config.toml      # linker rust-lld
├── docs/                   # manual de uso e este manual (pt/en), capturas de tela
├── tools/                  # modelo do manual em HTML offline
└── src/
    ├── main.rs             # janela (eframe), log de pânico
    ├── config.rs           # config.json, pastas (%APPDATA%\SoundMod), importacao da versao antiga
    ├── library.rs          # ordem dos sons e colecoes (funcoes puras, com testes)
    ├── i18n.rs             # textos pt/en, t()/tf(), idioma do Windows
    ├── codec.rs            # decodificar, reamostrar, gravar WAV
    ├── engine.rs           # mixer, canais, streams cpal, dispositivos
    ├── voice.rs            # efeitos de voz em tempo real
    ├── recorder.rs         # loopback + buffer dos últimos N s
    ├── hotkeys.rs          # gancho WH_KEYBOARD_LL, combinações, captura de tecla
    ├── logic.rs            # Core: ações do app (compartilhado entre UI e hotkeys)
    ├── icon_raster.rs      # desenho do ícone em pixels (janela e .exe)
    └── ui/
        ├── app.rs          # janela principal (SoundApp)
        ├── theme.rs        # paletas, fontes, estilo, brilho neon, temas
        ├── widgets.rs      # botões iluminados, tecla, interruptor, slider, medidor, cartão
        ├── waveform.rs     # forma de onda com alças
        ├── icons.rs        # ícones vetoriais
        ├── logo.rs         # logo do cabeçalho e ícone da janela
        └── window.rs       # barra de título e borda na cor do neon (DWM, Windows 11)
```

## 3. Módulos

### 3.1 `config.rs`
- **Pastas:** `Dirs { app, data }`. `app` = pasta do `.exe` (rodando de `target\debug|release`, a raiz do projeto). `data` = `%APPDATA%\SoundMod`, onde ficam `config.json`, `sounds\` e `clips\last_clip.wav`: substituir a pasta do programa numa atualização não apaga nada. É a **mesma pasta da versão Python**, que lê o mesmo arquivo.
- **Importação única:** se `data` ainda não tem `config.json` e existe um ao lado do `.exe` (versões antigas), copia `sounds\` e `clips\` sem sobrescrever o que já existe e, por último, o `config.json` (se algo falhar no meio, tenta de novo na próxima vez). Nada é apagado da pasta antiga. `load()` devolve a pasta de origem e a interface avisa na barra de status.
- **Config corrompido:** é copiado para `config-corrompido-AAAAMMDD-HHMMSS.json` antes de usar os padrões (senão o próximo save gravaria por cima). BOM do Bloco de Notas é aceito.
- Mesmo formato da versão Python: `language`, `mic_device`, `output_device`, `monitor_device`, `loopback_device`, `mix_mic`, `monitor`, `master_volume`, `clip_seconds`, `voice_effect`, `accent`, `compact_list`, `hear_self`, `clip_trim`, `clip_pitch`, `save_clip_key`, `play_clip_key`, `stop_key`, `collections[{name, collapsed}]`, `slots[{key, path, volume, collection}]`. **Campos desconhecidos são preservados** (`#[serde(flatten)] extra`), para uma versão não apagar o que a outra gravou. `collections` aceita também nomes soltos (`["Memes"]`).
- Dispositivos por **nome**: `None` = automático (resolvido na primeira abertura e então salvo), `Some("")` = nenhum. O texto antigo `"(nenhum)"` vira `""`.
- `resolve()`: caminhos relativos (`sounds/x.wav`) são relativos à pasta de dados.
- **Pasta fixa dos sons:** `import_sound(pasta, origem)` copia o arquivo para `sounds\` (o original fica intacto); um arquivo idêntico já lá é reaproveitado e um diferente com o mesmo nome vira `nome (2).ext`. `Core::adopt_slot` usa isso ao adicionar ou trocar o arquivo de um som (numa thread, antes de carregar) e grava o caminho relativo `sounds/<nome>`; `Core::adopt_all` faz o mesmo uma vez na abertura para os sons que ainda estão fora da pasta. O botão **Abrir pasta** (cartão de sons) abre `sounds\`.
- `save()` grava em `config.tmp` e renomeia.
- `Slot.id` só existe em memória: é o id do canal no mixer (e identifica o som na ordem/coleções, já que dois slots vazios são iguais em conteúdo).
- `load_in`/`save_in` recebem as pastas: os testes usam pastas temporárias.

### 3.1.1 `library.rs` (ordem e coleções)
Funções puras sobre a `Config`, portadas do `library.py`:
- A ordem de `cfg.slots` é a ordem na tela. Na tela vêm primeiro os sons sem coleção e depois cada coleção na ordem de `cfg.collections` (`groups()`).
- `move_slot(id, ±1)`: troca com o vizinho **da mesma coleção**. `set_collection(id, nome)`: põe o som no fim da coleção (erro se ela não existe).
- `clean_name()`: tira espaços extras; erro se vazio ou se já existe (sem diferenciar maiúsculas; ao renomear pode mudar só maiúsculas/minúsculas); os rótulos "(sem coleção)"/"(no collection)" de todos os idiomas são reservados.
- `rename_collection`, `delete_collection` (os sons vão para o fim de "sem coleção"; nenhum som é apagado), `move_collection`, `set_collapsed`.
- `normalize()`: completa configs antigas ou editadas à mão (nomes em branco, coleções repetidas, som apontando para coleção inexistente, que é criada).

### 3.2 `codec.rs`
- Formato interno: `Vec<f32>` estéreo intercalado a **48 kHz**.
- `decode()`: Symphonia → estéreo (mono duplica; multicanal usa frente + centro) → `resample()` para 48 kHz. Se o Symphonia não abrir, usa `ffmpeg.exe` (ao lado do programa ou no PATH), sem janela de console.
- `resample()`: sinc janelado polifásico (512 fases, Blackman), com corte reduzido ao baixar a taxa.
- `write_wav()`: PCM 16 bits.

### 3.3 `engine.rs`
`Engine` é compartilhado (`Arc`) entre interface, hotkeys e callbacks. Os `cpal::Stream` ficam com a interface (`SoundApp`); fechar = soltar os objetos (`Engine::close`).

**Streams:**
1. Microfone: formato nativo do dispositivo → mono → `LinearResampler` → `mic_buf`.
2. Saída para o cabo: **48 kHz estéreo f32** pedido direto; o cpal usa `AUTOCONVERTPCM`, então o WASAPI converte para o formato do dispositivo. Se o estéreo falhar, tenta o número de canais nativo.
3. Monitor (opcional): também 48 kHz; consome `mon_buf`.

**`render` (callback da saída):** soma os canais tocando × ganho × volume geral; puxa o mic e aplica o efeito de voz (com `catch_unwind`: um efeito com defeito é desligado em vez de derrubar o stream); alimenta o monitor (sons + voz se `hear_self`); mistura o mic se `mix_mic`; limita em [-1, 1]; publica os picos para os medidores.

**Canais:** `Channel { data, pos, start, end, gain, playing }` por `ChannelId` (`CLIP` = 0, slots ≥ 1). Só o trecho `[start, end)` toca.

| Método | Efeito |
|---|---|
| `set_sound(id, path, gain)` | decodifica (com cache) e cria o canal parado; devolve a duração |
| `play(id)` | se já toca, recomeça; senão continua de `pos` |
| `toggle(id)` | tocando → para; senão toca. `None` se o canal não existe |
| `pause(id)` | pausa/continua (parado no início: nada) |
| `stop` / `stop_all` | para e volta ao início do trecho |
| `seek`, `set_gain`, `set_range` | posição, volume ao vivo, corte |
| `set_pitch(id, semitons)` | velocidade de leitura `2^(st/12)` com interpolação linear (tom e duração mudam juntos); usado no clipe (`clip_pitch` na config, −12..12) e mantido ao recarregar o canal |
| `state(id)` | `(pos_s, duração_s, tocando)` |
| `channel_audio(id)` | dados + trecho (forma de onda, salvar como som) |

**Um som por vez:** `play`, `toggle` e continuar uma pausa chamam `solo`, que para os outros canais.
**Latência:** `mic_buf` e `mon_buf` descartam o excedente acima de 250 ms (relógios dos dispositivos divergem).
**Erros de dispositivo** (desconectou) ligam `device_lost`; a interface avisa para clicar em Aplicar.

### 3.4 `voice.rs`
Porte direto do `voice.py`, agora amostra a amostra (sem numpy). Cada efeito implementa `Effect::process(&mut [f32])` e guarda estado entre blocos de qualquer tamanho.

| Efeito | Como funciona |
|---|---|
| `deep` / `high` | `PitchShifter` (0,75 / 1,45): linha de atraso com dois cursores em contrafase e janelas sin² |
| `ogre` | pitch 0,62 + saturação `tanh` + passa-baixa FIR 2,2 kHz |
| `robot` | STFT 1024/256 com fase zerada (voz monótona ~188 Hz) |
| `autotune` | autocorrelação por FFT em 2048 amostras + histerese de 0,6 semitom |
| `echo` | 280 ms, realimentação 0,45 |
| `radio` | passa-banda FIR 400–3000 Hz (481 taps) + saturação |
| `alien` | pitch 1,3 com vibrato de 5 Hz |

**Novo efeito:** crie o tipo, registre em `make()` e `EFFECTS`, e a chave `fx_<nome>` em `logic::FX_KEYS` e nos dois idiomas do `i18n`.

### 3.5 `recorder.rs`
- No WASAPI, um stream de **entrada** num dispositivo de **saída** liga o loopback. Usa o formato nativo (taxa e canais) e guarda as amostras num `VecDeque` limitado a `seconds × taxa × canais`.
- `save()` grava os últimos N s em WAV na taxa nativa. Sem áudio tocando no PC não chegam amostras ("Nada gravado ainda").

### 3.6 `hotkeys.rs`
- Gancho `WH_KEYBOARD_LL` numa thread com laço de mensagens. O gancho só repassa `(tecla, apertou?)` por um canal e devolve na hora (o Windows remove ganchos lentos). Não bloqueia a tecla: ela continua chegando ao jogo.
- Thread de trabalho: ignora a repetição automática, monta a combinação (`ctrl+shift+a`, `f1`), confere modificadores "presos" com `GetAsyncKeyState` e dispara as ações registradas.
- `begin_capture()`: a próxima tecla vai para a interface em vez de disparar ações; **Esc cancela**. Apertar só um modificador continua esperando.
- Nomes estáveis em minúsculas; `normalize()` aceita sinônimos antigos (`control`, `escape`, `page_up`...).

### 3.7 `logic.rs` (`Core`)
O que a interface e as hotkeys fazem: tocar/parar slots, salvar/tocar o clipe, salvar o trecho como som, adicionar/remover sons (inclusive direto numa coleção: usa o primeiro slot vazio **do mesmo grupo** ou cria um no fim dele, e expande a coleção), mudar a ordem, criar/renomear/apagar/mover/recolher coleções (validando o nome com a mensagem já traduzida), trocar teclas, trocar o efeito, salvar a config. As hotkeys chamam `Core::on_action` **na própria thread**, então funcionam com a janela minimizada (o eframe nem chama `ui()` nesse estado). Trabalho demorado vai para threads; mensagens vão para `status`, que a interface lê a cada quadro.

### 3.8 Interface (`ui/`)
- **Modo imediato:** `SoundApp::ui` redesenha tudo a cada quadro (~30 fps, por causa das animações) lendo o estado do `Core`/`Engine`. As ações das linhas são coletadas num `Vec<RowAction>` e aplicadas depois do desenho.
- **Layout:** painel superior (logo, título neon, Ao vivo, medidores, cor, idioma, Parar tudo), barra de status embaixo (com o botão **Pasta de dados**), lateral fixa (Dispositivos, Efeito de voz) e área central com o clipe num painel inferior e os sons ocupando o resto.
- **Lista de sons:** desenhada por grupo (`library::groups`) a partir de uma cópia da config. Sem nenhuma coleção, fica sem cabeçalhos (como antes). Cada coleção tem um cabeçalho (seta/nome recolhem, **+ Som**, ▲▼, ✎, lixeira); coleção recolhida não desenha as linhas, mas as teclas continuam valendo. Cada linha tem ▲▼ empilhadas (desligadas nas pontas do grupo) e um botão de camadas com o menu de coleções (`egui::Popup::menu`), incluindo "+ Nova coleção" que já move o som para ela.
- **Lista compacta** (`compact_list` na config): o interruptor no cartão de sons troca o tamanho das linhas e dos cabeçalhos: botões de 22 px em vez de 30, botão de tecla de 24 px (`key_button_sized`), margens e espaçamento entre linhas mínimos, fonte um pouco menor.
- **Janelinha de nome** (`NameDialog`): salvar clipe como som, nova coleção e renomear coleção; erros de validação aparecem dentro dela, em vermelho, sem fechar.
- **Neon (`theme.rs`):** o brilho é uma `Shadow` colorida sem deslocamento atrás do elemento (`glow`); texto neon é o mesmo galley repintado com halo transparente. Paleta fixa: grafite `#0A0C0E`/`#121518` e vermelho neon só para remover/parar tudo.
- **Cores de base:** `Base` agrupa fundo, painéis, itens, bordas, textos, poços, trilhos e cabeçalhos. Há duas: `GRAPHITE` (cores de neon) e `DUSK` (violeta da noite, do tema Vice City). A interface nunca usa uma dessas cores fixa: chama `bg()`, `panel()`, `item()`, `border()`, `txt()`, `muted()`, `dim()`, `well()`, `track()`, `header_fill()`, que seguem a opção atual.
- **Tema Vice City:** a entrada `vice_city` de `ACCENTS` (o nome antigo `sunset` ainda é aceito na config) usa `DUSK`, o neon Coral Pink e o degrade `VICE_CITY` (Magenta, Coral Pink, Coral, Orange, Warm Yellow), com texto violeta sobre o neon. `spectrum_at(t)` dá a cor do degrade na posição (sem degrade, a cor do neon); `fill()` pinta um retângulo arredondado com uma textura 256x1 do degrade (criada em `apply_style`), usada nos botões principais, tecla tocando, efeito escolhido e trilho dos sliders. Título (letra a letra), forma de onda, medidores, luzes do fundo e barras do ícone usam `spectrum_at`. **Novo tema:** uma entrada em `ACCENTS` com `base` e `spectrum`.
- **Ícone:** as 5 barras têm cor própria (`icon_raster::BAR_COLORS`, arco-íris; no tema, `theme::bar_colors()` usa a paleta) e a moldura segue o neon. O `.exe` usa moldura verde e barras arco-íris; a janela já nasce com a cor salva (`config::peek_accent` em `main.rs`).
- **Cor do neon:** `ACCENTS` tem 8 cores (verde, ciano, azul, roxo, rosa, vermelho, laranja, amarelo), cada uma com o tubo (`neon`) e o núcleo claro (`core`). A atual fica num atômico; a interface nunca usa uma cor de destaque fixa, só `accent()`, `accent_core()`, `accent_deep()` (fundo tingido) e `on_accent()` (texto sobre o neon: escuro nas cores claras, branco nas escuras, pela luminância). Trocar a cor (`SoundApp::set_accent`) chama `set_accent`, reaplica o estilo do egui, recolore a borda da janela (`window::recolor`), troca o ícone da janela (`ViewportCommand::Icon`) e salva `accent` no `config.json`. O ícone do `.exe` continua verde. **Nova cor:** acrescente em `ACCENTS` e a chave `accent_<nome>` nos dois idiomas e em `i18n::accent_name`.
- **Fontes:** Segoe UI, Segoe UI Semibold e Bahnschrift (títulos) do Windows quando existem; senão as do egui. Ícones são vetoriais (`icons.rs`), sem fonte de ícones.
- **Widgets próprios:** `button` (pílula iluminada), `key_button` (pílula + ✎ + ⌫, com layout próprio para funcionar dentro de layouts invertidos), `toggle`, `slider`, `level_meter`, `status_pill`, `choice`, `card` (com filete de luz no topo), `Waveform`.
- **Janela:** borda na cor do neon e barra de título escura via `DwmSetWindowAttribute` (Windows 11; no 10 é ignorado).

### 3.9 Idiomas (`i18n.rs`)
- Tabelas `PT` e `EN` com as mesmas chaves; `t("chave")` e `tf("chave", &[("nome", &valor)])` devolvem o texto no idioma atual. Um teste garante que os dois idiomas têm as mesmas chaves e os mesmos `{campos}`.
- `language` na config: `"pt"`, `"en"` ou `null` = idioma do Windows (`GetUserDefaultUILanguage`: português → pt, qualquer outro → en). O menu do cabeçalho troca na hora e salva.
- **Novo idioma:** inclua em `LANGUAGES` e crie a tabela com as mesmas chaves. **Texto novo:** nunca escreva direto no código; crie a chave em todos os idiomas.

## 4. Threads

| Thread | O que faz | Regra |
|---|---|---|
| Principal (eframe) | interface, abrir/fechar streams | dona dos `cpal::Stream` |
| Callbacks do cpal | mic, saída, monitor, loopback | rápidos, sem I/O nem interface |
| `keyboard-hook` | gancho + laço de mensagens | só repassa eventos |
| `hotkeys` | combinações e ações (`Core::on_action`) | trabalho pesado vai para threads |
| Workers curtos | decodificar, gravar WAV | reportam por `Core::say` |

## 5. Compilação (por que GNU + rust-lld)

- O toolchain **GNU** não exige o Visual Studio. O `rust-toolchain.toml` fixa `stable-x86_64-pc-windows-gnu` com `llvm-tools`.
- As crates `windows*` importam funções por `raw-dylib`, que pede um **dlltool**. O do pacote GNU precisa do assembler `as`, que não vem junto; o `build.ps1` usa o `llvm-ar` do `llvm-tools` renomeado para `dlltool.exe` (ele vira o `llvm-dlltool`).
- O **GNU ld** embutido liga essas bibliotecas de importação de forma errada: o programa abre e quebra com acesso inválido (salto para um endereço de importação não resolvido). Com o **rust-lld** (`.cargo/config.toml`) a ligação sai certa.
- `build.ps1 -Package` gera o `.zip` da release: `SoundMod.exe`, `LICENSE.txt` e os manuais de uso em HTML offline (Markdown renderizado pela API `/markdown` do GitHub, com âncoras nos títulos, print embutido, dentro de `tools/manual-template.html`), e mostra os SHA-256.
- `build.rs` monta o `.res` (ícone 16–256 px + VERSIONINFO) em Rust e passa ao linker, sem `rc.exe`/`windres`.

## 6. Testes

```
.\build.ps1 -Test              # unitários
.\build.ps1 -Test -Hardware    # + teste real com o VB-CABLE
```

- `codec`: reamostragem (duração e frequência), WAV ida e volta.
- `engine`: tom do clipe (+12 = 880 Hz e metade da duração; −12 = 220 Hz), tocar/parar/solo, mixagem e fim do trecho, pausa/seek/trecho, mic → saída e monitor, limite de latência, taxa do reamostrador linear.
- `voice`: frequência do pitch shifter, todos os efeitos finitos e sem silenciar, silêncio continua silêncio, detecção de pitch, autotune puxando para o semitom.
- `hotkeys`: normalização, nomes ida e volta, teclas inválidas.
- `i18n`: mesmas chaves e campos nos dois idiomas.
- `library` (portados de `tests/test_library.py`): grupos na ordem, mover só dentro da coleção, pontas, slots vazios iguais (por id), mudar de coleção vai para o fim, renomear, apagar mantém os sons, mover/recolher coleção, validação de nomes, configs antigas e coleções repetidas/em branco.
- `config`: cópia para a pasta de sons (copia, reaproveita idêntico, renomeia no conflito); e os portados de `tests/test_config.py`: instalação nova, importação da versão antiga (config, sons e clipe; nada apagado), pasta do programa substituída mantém tudo, config antigo copiado de novo não vence, importação não sobrescreve sons, config corrompido guardado à parte, ida e volta preservando campos desconhecidos.
- `cable_roundtrip` (ignorado por padrão): toca 1 kHz baixo no `CABLE Input`, grava pelo loopback do mesmo dispositivo e confere frequência e amplitude no WAV salvo. Não sai nos alto-falantes.

### Variáveis de ambiente (diagnóstico)
- `SOUNDMOD_DATA_DIR`: usa outra pasta de dados no lugar de `%APPDATA%\SoundMod` (testes, capturas de tela sem mexer nos dados reais).
- `SOUNDMOD_DEBUG_KEYS=1`: mostra no stderr cada tecla que o gancho recebe e cada combinação montada (rodar o build de debug pelo terminal).

**Teclas simuladas em testes:** processos abertos dentro de um ambiente isolado (sandbox de ferramentas de automação) podem não receber no gancho as teclas injetadas com `keybd_event`/`SendInput`. Aberto normalmente (Explorer, atalho, terminal do usuário) o gancho recebe; para testar teclas globais automaticamente, abra o app fora do sandbox.

## 7. Estado e pendências

Verificado nesta máquina: abertura de mic/cabo/fones e loopback, `cable_roundtrip` (1000 Hz, pico 0,100), F1 e F5 globais pelo gancho (tocou o slot; salvou o clipe), interface em pt e en, ícone e versão no `.exe`. Ordem e coleções pela interface: ▲▼, menu de coleção, nova coleção (nome repetido recusado com aviso; nome válido criado), recolher, tudo salvo no `config.json`; importação da config antiga com o aviso na barra de status.

Ainda não verificado:
1. Efeitos de voz com voz real, de ouvido (só sinais sintéticos nos testes).
2. Captura de tecla pela interface (✎) com teclado real.
3. Detecção automática do idioma num Windows em inglês.

Ideias: bandeja do sistema, iniciar com o Windows, arrastar e soltar arquivos, fade-out ao parar, intensidade ajustável dos efeitos, renomear sons salvos.
