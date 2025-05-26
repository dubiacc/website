// Script to replace ligatures since they don't render correctly
const replaceLigatures = () => {
  const replacements = new Map([
    ['œ', 'oe'],
    ['Œ', 'Oe'],
    ['æ', 'ae'],
    ['Æ', 'Ae']
  ]);

  const replaceInTextNode = (node) => {
    if (!node.textContent.match(/[œŒæÆ]/)) return;
    
    let text = node.textContent;
    replacements.forEach((replacement, ligature) => {
      text = text.replaceAll(ligature, replacement);
    });
    node.textContent = text;
  };

  const processNode = (node) => {
    if (node.nodeType === Node.TEXT_NODE) {
      replaceInTextNode(node);
      return;
    }
    
    if (node.nodeType !== Node.ELEMENT_NODE) return;
    
    [...node.childNodes].forEach(processNode);
  };

  processNode(document.body);
};

document.addEventListener('DOMContentLoaded', replaceLigatures);