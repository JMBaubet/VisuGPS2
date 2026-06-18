export function renderMarkdown(markdown: string): string {
  if (!markdown) return '';
  
  // Échapper le HTML d'origine pour éviter les failles XSS
  let html = markdown
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');

  // Rétablir l'échappement pour les balises générées ci-dessous (on le fait à la volée ou lors de la substitution)
  // Gras: **texte** ou __texte__
  html = html.replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>');
  html = html.replace(/__(.*?)__/g, '<strong>$1</strong>');

  // Italique: *texte* ou _texte_
  html = html.replace(/\*(.*?)\*/g, '<em>$1</em>');
  html = html.replace(/_(.*?)_/g, '<em>$1</em>');

  // Code: `code`
  html = html.replace(/`(.*?)`/g, '<code class="markdown-code">$1</code>');

  // Liens: [texte](url)
  // On doit convertir les &amp; dans l'URL pour ne pas casser le lien
  html = html.replace(/\[(.*?)\]\((.*?)\)/g, (_match, text, url) => {
    const cleanUrl = url.replace(/&amp;/g, '&');
    return `<a href="${cleanUrl}" target="_blank" rel="noopener noreferrer">${text}</a>`;
  });

  // Listes à puces: lignes commençant par "- " ou "* "
  const lines = html.split('\n');
  let inList = false;
  const processedLines: string[] = [];
  
  for (let line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith('- ') || trimmed.startsWith('* ')) {
      const content = trimmed.substring(2);
      if (!inList) {
        inList = true;
        processedLines.push('<ul>');
      }
      processedLines.push(`<li>${content}</li>`);
    } else {
      if (inList) {
        inList = false;
        processedLines.push('</ul>');
      }
      processedLines.push(line);
    }
  }
  
  if (inList) {
    processedLines.push('</ul>');
  }
  
  html = processedLines.join('\n');

  // Retours à la ligne (sauf après les éléments de bloc de liste pour éviter les espaces verticaux excessifs)
  html = html.replace(/<\/ul>\n/g, '</ul>');
  html = html.replace(/<\/li>\n/g, '</li>');
  html = html.replace(/\n/g, '<br>');

  return html;
}
