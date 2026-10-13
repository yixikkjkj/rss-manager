const path = require('path');
const { createHash } = require('node:crypto');
const { HtmlRspackPlugin, CssExtractRspackPlugin } = require('@rspack/core');

module.exports = (_env, argv) => {
  const production = argv.mode === 'production';
  const cssLoaders = (scss) => [
    production ? CssExtractRspackPlugin.loader : 'style-loader',
    {
      loader: 'css-loader',
      options: {
        importLoaders: scss ? 1 : 0,
        modules: {
          auto: /\.module\.(css|scss)$/i,
          namedExport: false,
          getLocalIdent(context, _template, localName) {
            const relative = path.relative(__dirname, context.resourcePath).split(path.sep).join('/');
            const component = path.basename(context.resourcePath).replace(/\.module\.(css|scss)$/i, '');
            const hash = createHash('sha256').update(`${relative}:${localName}`).digest('hex').slice(0, 6);
            return `${component}-${hash}-${localName}`;
          },
        },
      },
    },
    ...(scss
      ? [
          {
            loader: 'sass-loader',
            options: { api: 'modern-compiler', implementation: require('sass-embedded') },
          },
        ]
      : []),
  ];
  return {
    entry: './src/Main.ts',
    output: {
      path: path.resolve(__dirname, 'dist'),
      filename: 'assets/[name].[contenthash].js',
      chunkFilename: 'assets/[name].[contenthash].js',
      clean: true,
      publicPath: '/',
    },
    resolve: { extensions: ['.ts', '.js'] },
    module: {
      rules: [
        {
          test: /\.ts$/,
          exclude: /node_modules/,
          use: {
            loader: 'builtin:swc-loader',
            options: { jsc: { parser: { syntax: 'typescript' }, target: 'es2020' } },
          },
          type: 'javascript/auto',
        },
        { test: /\.css$/i, use: cssLoaders(false), type: 'javascript/auto' },
        { test: /\.scss$/i, use: cssLoaders(true), type: 'javascript/auto' },
      ],
    },
    plugins: [
      new HtmlRspackPlugin({ template: './index.html' }),
      ...(production
        ? [
            new CssExtractRspackPlugin({
              filename: 'assets/[name].[contenthash].css',
              chunkFilename: 'assets/[name].[contenthash].css',
            }),
          ]
        : []),
    ],
    devServer: {
      historyApiFallback: true,
      port: 3000,
      proxy: [{ context: ['/api'], target: 'http://127.0.0.1:8080' }],
      hot: false,
      liveReload: true,
    },
  };
};
