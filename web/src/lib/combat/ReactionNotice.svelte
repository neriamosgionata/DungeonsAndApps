<script lang="ts">
  let {
    notice,
    onClose,
  }: {
    notice: { type: string; message: string };
    onClose: () => void;
  } = $props();
</script>

<div class="reaction-notice">
  <span class="msg">⚡ {notice.message}</span>
  <button type="button" class="ca-btn close" onclick={onClose}>✕</button>
</div>

<style>
  .reaction-notice {
    background: rgba(201, 168, 76, 0.2);
    border: 1px solid #c9a84c;
    border-radius: 4px;
    padding: 0.5rem 0.75rem;
    margin-bottom: 0.5rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .msg { color: #c9a84c; font-weight: 600; }
  .close { font-size: 0.7rem; padding: 0.15rem 0.5rem; }
</style>
