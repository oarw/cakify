<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  type Ready = {
    port: number;
    protocol_version: string;
    fixture_hash: string;
    session_token: string;
    pid: number;
  };

  type Message = {
    role: string;
    markdown: string;
  };

  let dark = false;
  let status = '等待 core';
  let ready: Ready | null = null;
  let messages: Message[] = [];
  let scrollTop = 0;
  let viewportHeight = 480;

  const total = 10_000;
  const rowHeight = 76;

  $: visibleStart = Math.max(0, Math.floor(scrollTop / rowHeight) - 4);
  $: visibleCount = Math.ceil(viewportHeight / rowHeight) + 10;
  $: visibleEnd = Math.min(total, visibleStart + visibleCount);
  $: visibleRows = Array.from({ length: visibleEnd - visibleStart }, (_, offset) => visibleStart + offset);

  onMount(async () => {
    ready = await invoke<Ready | null>('core_info');
    if (!ready) {
      status = '未连接 core · 静态 UI 模式';
      return;
    }
    status = 'core ready · 载入首个分页中';
    try {
      const response = await fetch(`http://127.0.0.1:${ready.port}/fixture/messages?offset=0&limit=200`, {
        headers: { 'x-cakify-session': ready.session_token },
      });
      const payload = await response.json();
      messages = payload.messages as Message[];
      status = 'core ready · 已载入首个分页';
    } catch (error) {
      status = `core 请求失败：${String(error)}`;
    }
  });

  function toggleTheme() {
    dark = !dark;
  }

  async function runFixture() {
    if (!ready) {
      status = 'core 未连接';
      return;
    }
    status = '工具时间线运行中 · 可取消';
    try {
      await fetch(`http://127.0.0.1:${ready.port}/run/cancel`, {
        method: 'POST',
        headers: {
          'content-type': 'application/json',
          'x-cakify-session': ready.session_token,
        },
        body: JSON.stringify({ run_id: 'tauri-fixture' }),
      });
      status = '工具时间线已发送取消';
    } catch (error) {
      status = `core 请求失败：${String(error)}`;
    }
  }

  function messageAt(index: number): Message {
    return messages[index] ?? { role: 'fixture', markdown: `加载 fixture 消息 ${String(index).padStart(5, '0')}` };
  }
</script>

<svelte:head>
  <title>Cakify · Tauri benchmark</title>
</svelte:head>

<div class:dark class="shell">
  <aside class="sidebar">
    <h1>Cakify</h1>
    <div class="workspace-card">
      <strong>Benchmark workspace</strong>
      <span>Tauri + Svelte + Rust</span>
    </div>
    <span class="section-label">会话</span>
    {#each Array(6) as _, index}
      <div class:active={index === 0} class="session-row">{index === 0 ? '10k fixture / active' : 'Archived conversation'}</div>
    {/each}
    <div class="sidebar-spacer"></div>
    <small>同一 Rust core · deterministic fixture</small>
  </aside>

  <main class="content">
    <header class="header">
      <h2>New conversation</h2>
      <div class="header-actions">
        <button class="secondary" on:click={toggleTheme}>{dark ? '亮色' : '暗色'}</button>
        <button class="primary" on:click={runFixture}>运行 fixture</button>
      </div>
    </header>

    <section
      class="message-viewport"
      bind:clientHeight={viewportHeight}
      on:scroll={(event) => (scrollTop = (event.currentTarget as HTMLElement).scrollTop)}
    >
      <div class="message-spacer" style={`height: ${total * rowHeight}px`}>
        <div class="message-window" style={`transform: translateY(${visibleStart * rowHeight}px)`}>
          {#each visibleRows as index (index)}
            {@const message = messageAt(index)}
            <article class:alternate={index % 4 !== 0} class="message-card">
              <span class="role">{message.role}</span>
              <span class="separator">·</span>
              <span>{message.markdown.replaceAll('\n', ' ')}</span>
            </article>
          {/each}
        </div>
      </div>
    </section>

    <footer class="composer-wrap">
      <textarea rows="4" placeholder="输入消息… 支持中文 IME"></textarea>
      <div class="composer-meta">
        <span>{status}</span>
        <div><button class="icon-button" aria-label="附件">＋</button><button class="send-button" aria-label="发送">↑</button></div>
      </div>
    </footer>
  </main>
</div>
