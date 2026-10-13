/** Keep HTML rendering behind SafeHtml, including Lit property bindings. */
const htmlProperties = new Set(['innerHTML', 'outerHTML']);
const propertyName = (node) => {
  if (node?.type !== 'MemberExpression') return undefined;
  return node.computed ? node.property.value : node.property.name;
};

export default {
  meta: { name: 'rss' },
  rules: {
    'no-html-sinks': {
      meta: {
        type: 'problem',
        schema: [],
        messages: { unsafe: 'Render HTML through Lit templates or components/SafeHtml/SafeHtml.' },
      },
      create(context) {
        const report = (node) => context.report({ node, messageId: 'unsafe' });
        return {
          AssignmentExpression(node) {
            if (htmlProperties.has(propertyName(node.left))) report(node);
          },
          CallExpression(node) {
            if (propertyName(node.callee) === 'insertAdjacentHTML') report(node);
          },
          TemplateElement(node) {
            if (/\.(innerHTML|outerHTML)\s*=/.test(node.value.raw)) report(node);
          },
        };
      },
    },
  },
};
