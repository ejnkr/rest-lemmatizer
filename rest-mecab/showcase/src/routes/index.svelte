<script type="ts">
  interface Morph {
    token: string;
    tag: string;
  }
	interface Token {
		token: string;
		tags: string[];
		symantic_group: string | null;
		has_support: boolean | null;
		pronounce: string | null;
		kind: string | null;
		left_tag: string | null;
		right_tag: string | null;
		morphemes: Morph[] | null;
	}
	let query: string = '';
	let tokens: Token[] = [];
	let timer = null;
	function search() {
		if (timer) {
			clearTimeout(timer);
			timer = null;
		}
		timer = setTimeout(() => {
			fetch(`/tokenize?q=${query}`)
				.then((res) => res.json())
				.then((res) => (tokens = res));
			console.log('hi');
		}, 100);
	}
</script>

<div class="container m-auto m-8">
  <div class='flex items-baseline'>
    <h1 class="text-xl border-l-4 border-green-400 pl-2 w-52">형태소 분석기 쇼케이스</h1>
    <a class="text-blue-500 ml-4 text-xs" href='/userdic-nouns'> 등록된(혹은 곧 등록될) 명사 리스트 </a>
  </div>
	<div class="mt-8">
		<input
			class="border shadow rounded py-2 px-4 w-96"
			bind:value={query}
			placeholder="트위치에 채팅하듯이 입력해보세요"
			on:keyup={search}
		/>
		<!-- <button class="ml-2 border-2 rounded-full border-green-300 py-2 px-4" on:click={search}> 검색 </button> -->
	</div>
	<div class="flex flex-row mt-6 text-sm">
		{#each tokens as { token, tags, morphemes }}
			<div
				class="flex items-baseline rounded-full p-2 px-3 text-white mr-2"
				class:bg-green-500={tags[0][0] == 'N'}
				class:bg-blue-500={tags[0][0] == 'V'}
				class:bg-purple-500={tags[0][0] == 'J'}
				class:bg-gray-500={['N', 'V', 'J'].indexOf(tags[0][0]) < 0}
			>
				<span> {token} </span>
				<span class="ml-1"> {tags.join('/')} </span>
				{#if morphemes}
					<span class="mx-1"> ~ </span>
					{#each morphemes as { token, tag }}
						<span class="flex divide-green-300 text-xs mr-1">
							<span> {token} </span>
							<span class="ml-0.5"> {tag} </span>
						</span>
					{/each}
				{/if}
			</div>
		{/each}
	</div>
</div>
