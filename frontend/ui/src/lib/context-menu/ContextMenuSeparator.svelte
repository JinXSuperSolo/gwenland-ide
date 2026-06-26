<!-- Visual divider between action groups. <hr> already carries the implicit
     "separator" role assistive tech announces (Requirement 3.5). -->
<hr class="cm-separator" />

<style>
  /* 1px muted line; inherits --cm-separator from the portal's token set. */
  .cm-separator {
    height: 0;
    margin: 4px 4px;
    border: none;
    border-top: 1px solid var(--cm-separator, rgba(255, 255, 255, 0.06));
  }
</style>
