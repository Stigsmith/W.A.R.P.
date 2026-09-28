<script lang="ts">
  // A glowing warpstone shard: the app's own icon, used instead of emoji.
  let { size = 18, pulse = false }: { size?: number; pulse?: boolean } = $props();
</script>

<svg class="shard" class:pulse width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
  <path d="M12 1.5 18.5 8.2 15.6 22.5H8.4L5.5 8.2Z" class="body" />
  <path d="M12 1.5 13.4 9.4 12 22.5 10.6 9.4Z" class="facet" />
  <path d="M5.5 8.2 10.6 9.4 8.4 22.5ZM18.5 8.2 13.4 9.4 15.6 22.5Z" class="edge" />
</svg>

<style>
  .shard {
    flex: none;
    overflow: visible;
    filter: drop-shadow(0 0 3px rgb(168 242 63 / 0.9)) drop-shadow(0 0 9px rgb(168 242 63 / 0.45));
  }

  .body {
    fill: #7fd62a;
  }

  .facet {
    fill: #e2ffab;
    opacity: 0.85;
  }

  .edge {
    fill: #3f7a14;
    opacity: 0.7;
  }

  .pulse {
    animation: shard-pulse 2.4s ease-in-out infinite;
  }

  @keyframes shard-pulse {
    50% {
      filter: drop-shadow(0 0 5px rgb(202 255 107 / 1)) drop-shadow(0 0 16px rgb(168 242 63 / 0.7));
    }
  }
</style>
