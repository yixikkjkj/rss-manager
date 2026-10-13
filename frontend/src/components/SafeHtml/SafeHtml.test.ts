import { render } from 'lit';
import { expect, it } from 'vitest';
import { safeHtml, safeHref } from './SafeHtml';
import { highlight } from '../Highlight/Highlight';

it('removes scripts, handlers, forms, styles and unsafe links while preserving readable markup', () => {
  const container = document.createElement('div');
  document.body.append(container);
  render(
    safeHtml(
      '<p>Hello <strong>world</strong></p><img src="https://example.com/x" onerror="alert(1)"><a href="javascript:alert(1)">bad</a><iframe src="/"></iframe><script>alert(1)</script><style>body{display:none}</style><form><input></form><p style="position:fixed">text</p>',
    ),
    container,
  );
  expect(container.querySelector('strong')).toHaveTextContent('world');
  expect(container.querySelector('img')).not.toHaveAttribute('onerror');
  expect(container.querySelector('a')).not.toHaveAttribute('href');
  expect(container.querySelector('script,iframe,style,form,input,[style]')).toBeNull();
  expect(safeHref('javascript:alert(1)')).toBeUndefined();
});
it('highlights text without interpreting HTML', () => {
  const container = document.createElement('div');
  document.body.append(container);
  render(highlight('hello <img src=x onerror=alert(1)> world', 'world'), container);
  expect(container.querySelector('img')).toBeNull();
  expect(container.querySelector('mark')).toHaveTextContent('world');
});
